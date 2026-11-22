// FlowLens WFP Callout Driver - Full Implementation
// FlowLens WFP Callout Driver - Full Implementation
// Proxyless system-wide traffic capture on Windows 10/11 via WFP callouts.

#include "flowlens_driver.h"

#include <ntddk.h>
#include <ndis.h>
#include <fwpsk.h>
#include <fwpmk.h>
#include <wdmsec.h>
#include <in6addr.h>

#ifdef DBG
#define TRACE(fmt, ...) DbgPrintEx(DPFLTR_IHDPDX_ID, DPFLTR_INFO_LEVEL, "[FlowLens] " fmt "\n", __VA_ARGS__)
#else
#define TRACE(fmt, ...)
#endif

// ---------------------------------------------------------------------------
// Globals
// ---------------------------------------------------------------------------
static PDEVICE_OBJECT g_DeviceObject = NULL;
static UNICODE_STRING g_DeviceName = RTL_CONSTANT_STRING(L"\\Device\\FlowLens");
static UNICODE_STRING g_SymLinkName = RTL_CONSTANT_STRING(L"\\DosDevices\\FlowLens");

static const GUID FLOWLENS_CALLOUT_V4_GUID =
    { 0x12345678, 0x9abc, 0xdef0, { 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88 } };
static const GUID FLOWLENS_SUBLAYER_GUID =
    { 0x87654321, 0xcba9, 0x0fed, { 0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11 } };

// ---------------------------------------------------------------------------
// Ring buffer
// ---------------------------------------------------------------------------
NTSTATUS FlowLensRingBufferInit(PFLOWLENS_RING_BUFFER *ring_out) {
    PFLOWLENS_RING_BUFFER r = (PFLOWLENS_RING_BUFFER)ExAllocatePool2(
        POOL_FLAG_NON_PAGED, sizeof(FLOWLENS_RING_BUFFER), 'LFwF');
    if (!r) return STATUS_INSUFFICIENT_RESOURCES;
    r->buffer = (PUCHAR)ExAllocatePool2(POOL_FLAG_NON_PAGED, RING_BUFFER_SIZE, 'LFwF');
    if (!r->buffer) { ExFreePool(r); return STATUS_INSUFFICIENT_RESOURCES; }
    r->size = RING_BUFFER_SIZE;
    r->write_offset = 0;
    r->read_offset = 0;
    KeInitializeSpinLock(&r->lock);
    KeInitializeEvent(&r->data_available, NotificationEvent, FALSE);
    *ring_out = r;
    TRACE("Ring buffer initialized");
    return STATUS_SUCCESS;
}

VOID FlowLensRingBufferFree(PFLOWLENS_RING_BUFFER ring) {
    if (!ring) return;
    if (ring->buffer) ExFreePool(ring->buffer);
    ExFreePool(ring);
}

// placeholder
NTSTATUS FlowLensRingBufferWrite(PFLOWLENS_RING_BUFFER ring,
    PFLOWLENS_TRAFFIC_EVENT event, const PUCHAR payload, ULONG payload_len) {
    KIRQL old_irql;
    ULONG total = sizeof(FLOWLENS_TRAFFIC_EVENT) + payload_len;
    ULONG w, r, avail;
    if (total > ring->size / 2) return STATUS_BUFFER_TOO_SMALL;
    KeAcquireSpinLock(&ring->lock, &old_irql);
    w = ring->write_offset;
    r = ring->read_offset;
    avail = (w >= r) ? (ring->size - (w - r) - 1) : (r - w - 1);
    if (avail < total) {
        KeReleaseSpinLock(&ring->lock, old_irql);
        return STATUS_BUFFER_OVERFLOW;
    }
    ULONG written = 0;
    written += min(sizeof(FLOWLENS_TRAFFIC_EVENT), ring->size - w);
    RtlCopyMemory(ring->buffer + w, event, written);
    if (written < sizeof(FLOWLENS_TRAFFIC_EVENT)) {
        RtlCopyMemory(ring->buffer, (PUCHAR)event + written, sizeof(FLOWLENS_TRAFFIC_EVENT) - written);
        written = sizeof(FLOWLENS_TRAFFIC_EVENT);
    }
    if (payload_len > 0) {
        ULONG pos = (w + sizeof(FLOWLENS_TRAFFIC_EVENT)) % ring->size;
        ULONG copy1 = min(payload_len, ring->size - pos);
        RtlCopyMemory(ring->buffer + pos, payload, copy1);
        if (copy1 < payload_len) {
            RtlCopyMemory(ring->buffer, payload + copy1, payload_len - copy1);
        }
    }
    ring->write_offset = (w + total) % ring->size;
    KeReleaseSpinLock(&ring->lock, old_irql);
    KeSetEvent(&ring->data_available, IO_NO_INCREMENT, FALSE);
    return STATUS_SUCCESS;
}

NTSTATUS FlowLensRingBufferRead(PFLOWLENS_RING_BUFFER ring, PUCHAR output,
    ULONG output_size, PULONG bytes_read) {
    KIRQL old_irql;
    ULONG w, r, avail, to_read;
    KeAcquireSpinLock(&ring->lock, &old_irql);
    w = ring->write_offset;
    r = ring->read_offset;
    avail = (w >= r) ? (w - r) : (ring->size - r + w);
    if (avail == 0) {
        KeReleaseSpinLock(&ring->lock, old_irql);
        *bytes_read = 0;
        return STATUS_NO_MORE_ENTRIES;
    }
    to_read = min(avail, output_size);
    ULONG copy1 = min(to_read, ring->size - r);
    RtlCopyMemory(output, ring->buffer + r, copy1);
    if (copy1 < to_read) {
        RtlCopyMemory(output + copy1, ring->buffer, to_read - copy1);
    }
    ring->read_offset = (r + to_read) % ring->size;
    KeReleaseSpinLock(&ring->lock, old_irql);
    *bytes_read = to_read;
    return STATUS_SUCCESS;
}
VOID NTAPI FlowLensClassify(
    _In_ const FWPS_INCOMING_VALUES *inFixedValues,
    _In_ const FWPS_INCOMING_METADATA_VALUES *inMetaValues,
    _Inout_ void *layerData,
    _In_ const void *classifyContext,
    _In_ const FWPS_FILTER *filter,
    _In_ UINT64 flowContext,
    _Inout_ FWPS_CLASSIFY_OUT *classifyOut
) {
    PFLOWLENS_DEVICE_EXTENSION ext = NULL;
    FLOWLENS_TRAFFIC_EVENT evt;
    UNREFERENCED_PARAMETER(layerData);
    UNREFERENCED_PARAMETER(classifyContext);
    UNREFERENCED_PARAMETER(filter);
    UNREFERENCED_PARAMETER(flowContext);
    UNREFERENCED_PARAMETER(inMetaValues);

    RtlZeroMemory(&evt, sizeof(evt));
    evt.timestamp = KeQueryInterruptTime();
    evt.pid = (ULONG)(ULONG_PTR)PsGetCurrentProcessId();

    PEPROCESS proc = PsGetCurrentProcess();
    if (proc) {
        PCHAR name = (PCHAR)PsGetProcessImageFileName(proc);
        if (name) {
            SIZE_T n = 0;
            while (n < 259 && name[n]) n++;
            RtlCopyMemory(evt.process_name, name, n);
        }
    }

    if (inFixedValues) {
        for (UINT32 i = 0; i < inFixedValues->valueCount; i++) {
            const FWPS_FIELD *v = &inFixedValues->incomingValue[i];
            switch (v->fieldId) {
                case FWPS_FIELD_ALE_AUTH_CONNECT_IP_LOCAL_ADDRESS:
                    if (v->value.type == FWP_V4_ADDR_IP) evt.src_addr = v->value.uint32;
                    break;
                case FWPS_FIELD_ALE_AUTH_CONNECT_IP_REMOTE_ADDRESS:
                    if (v->value.type == FWP_V4_ADDR_IP) evt.dst_addr = v->value.uint32;
                    break;
                case FWPS_FIELD_ALE_AUTH_CONNECT_IP_LOCAL_PORT:
                    evt.src_port = (USHORT)v->value.uint16;
                    break;
                case FWPS_FIELD_ALE_AUTH_CONNECT_IP_REMOTE_PORT:
                    evt.dst_port = (USHORT)v->value.uint16;
                    break;
                case FWPS_FIELD_ALE_AUTH_CONNECT_IP_PROTOCOL:
                    evt.protocol = (UCHAR)v->value.uint8;
                    break;
            }
        }
    }
    evt.direction = FLOWLENS_DIR_OUTBOUND;
    evt.data_size = 0;

    if (g_DeviceObject) {
        ext = (PFLOWLENS_DEVICE_EXTENSION)g_DeviceObject->DeviceExtension;
        if (ext && ext->capturing && ext->ring_buffer) {
            NTSTATUS st = FlowLensRingBufferWrite(ext->ring_buffer, &evt, NULL, 0);
            if (st == STATUS_BUFFER_OVERFLOW)
                InterlockedIncrement((volatile LONG *)&ext->events_dropped);
            else if (NT_SUCCESS(st))
                InterlockedIncrement((volatile LONG *)&ext->events_written);
        }
    }

    classifyOut->actionType = FWP_ACTION_PERMIT;
    classifyOut->rights = 0;
    classifyOut->flags = 0;
}

NTSTATUS NTAPI FlowLensNotify(FWPS_CALLOUT_NOTIFY_TYPE notifyType,
    const GUID *filterKey, const FWPS_FILTER *filter) {
    UNREFERENCED_PARAMETER(notifyType);
    UNREFERENCED_PARAMETER(filterKey);
    UNREFERENCED_PARAMETER(filter);
    return STATUS_SUCCESS;
}

VOID NTAPI FlowLensDelete(UINT16 layerId, UINT32 calloutId) {
    UNREFERENCED_PARAMETER(layerId);
    UNREFERENCED_PARAMETER(calloutId);
}
NTSTATUS FlowLensRegisterCallouts(PDEVICE_OBJECT DeviceObject) {
    PFLOWLENS_DEVICE_EXTENSION ext = (PFLOWLENS_DEVICE_EXTENSION)DeviceObject->DeviceExtension;
    NTSTATUS status;
    FWPS_CALLOUT callout = { 0 };
    FWPM_SESSION session = { 0 };
    FWPM_CALLOUT fwpmCallout = { 0 };
    FWPM_FILTER fwpmFilter = { 0 };
    FWPM_SUBLAYER sublayer = { 0 };
    HANDLE engineHandle = NULL;
    FWPM_FILTER_ID filterId = 0;

    status = FwpmEngineOpen(NULL, RPC_C_AUTHN_WINNT, NULL, &session, &engineHandle);
    if (!NT_SUCCESS(status)) { TRACE("FwpmEngineOpen failed 0x%08x", status); return status; }
    ext->wfp_engine_handle = engineHandle;

    status = FwpmTransactionBegin(engineHandle, 0);
    if (!NT_SUCCESS(status)) goto cleanup;

    sublayer.subLayerKey = FLOWLENS_SUBLAYER_GUID;
    sublayer.displayData.name = L"FlowLens Sublayer";
    sublayer.displayData.description = L"FlowLens traffic capture sublayer";
    sublayer.weight = 0x100;
    FwpmSubLayerAdd(engineHandle, &sublayer, NULL);

    callout.calloutKey = FLOWLENS_CALLOUT_V4_GUID;
    callout.classifyFn = FlowLensClassify;
    callout.notifyFn = FlowLensNotify;
    callout.deleteFn = FlowLensDelete;
    status = FwpsCalloutRegister(DeviceObject, &callout, &ext->callout_id_v4);
    if (!NT_SUCCESS(status)) { TRACE("FwpsCalloutRegister failed 0x%08x", status); goto cleanup; }

    fwpmCallout.calloutKey = FLOWLENS_CALLOUT_V4_GUID;
    fwpmCallout.displayData.name = L"FlowLens ALE Connect Callout V4";
    fwpmCallout.displayData.description = L"FlowLens proxyless capture callout";
    fwpmCallout.applicableLayer = FWPM_LAYER_ALE_AUTH_CONNECT_V4;
    fwpmCallout.calloutId = ext->callout_id_v4;
    status = FwpmCalloutAdd(engineHandle, &fwpmCallout, NULL, NULL);
    if (!NT_SUCCESS(status)) { TRACE("FwpmCalloutAdd failed 0x%08x", status); goto cleanup; }

    fwpmFilter.filterKey = FLOWLENS_CALLOUT_V4_GUID;
    fwpmFilter.displayData.name = L"FlowLens Capture Filter V4";
    fwpmFilter.displayData.description = L"Capture all outbound connections";
    fwpmFilter.layerKey = FWPM_LAYER_ALE_AUTH_CONNECT_V4;
    fwpmFilter.subLayerKey = FLOWLENS_SUBLAYER_GUID;
    fwpmFilter.weight.type = FWP_UINT8;
    fwpmFilter.weight.uint8 = 0x0F;
    fwpmFilter.numFilterConditions = 0;
    fwpmFilter.filterCondition = NULL;
    fwpmFilter.action.type = FWP_ACTION_CALLOUT_INSPECTION;
    fwpmFilter.action.calloutKey = FLOWLENS_CALLOUT_V4_GUID;
    status = FwpmFilterAdd(engineHandle, &fwpmFilter, NULL, &filterId);
    if (!NT_SUCCESS(status)) { TRACE("FwpmFilterAdd failed 0x%08x", status); goto cleanup; }
    ext->filter_id_v4 = filterId;

    status = FwpmTransactionCommit(engineHandle);
    if (!NT_SUCCESS(status)) goto cleanup;

    TRACE("WFP callouts registered");
    return STATUS_SUCCESS;

cleanup:
    if (engineHandle) FwpmEngineClose(engineHandle);
    return status;
}

VOID FlowLensUnregisterCallouts(PDEVICE_OBJECT DeviceObject) {
    PFLOWLENS_DEVICE_EXTENSION ext = (PFLOWLENS_DEVICE_EXTENSION)DeviceObject->DeviceExtension;
    if (ext && ext->wfp_engine_handle) {
        FwpmTransactionBegin(ext->wfp_engine_handle, 0);
        if (ext->filter_id_v4) FwpmFilterDeleteById(ext->wfp_engine_handle, ext->filter_id_v4);
        if (ext->callout_id_v4) FwpsCalloutUnregisterByKey(&FLOWLENS_CALLOUT_V4_GUID);
        FwpmTransactionCommit(ext->wfp_engine_handle);
        FwpmEngineClose(ext->wfp_engine_handle);
        ext->wfp_engine_handle = NULL;
    }
}
NTSTATUS FlowLensStartCapture(PDEVICE_OBJECT DeviceObject) {
    PFLOWLENS_DEVICE_EXTENSION ext = (PFLOWLENS_DEVICE_EXTENSION)DeviceObject->DeviceExtension;
    if (ext->capturing) return STATUS_SUCCESS;
    if (!ext->ring_buffer) {
        NTSTATUS st = FlowLensRingBufferInit(&ext->ring_buffer);
        if (!NT_SUCCESS(st)) return st;
    }
    ext->events_dropped = 0;
    ext->events_written = 0;
    ext->active_filters = ext->filter_count;
    KeQuerySystemTime(&ext->start_time);
    ext->capturing = TRUE;
    TRACE("Capture started");
    return STATUS_SUCCESS;
}

NTSTATUS FlowLensStopCapture(PDEVICE_OBJECT DeviceObject) {
    PFLOWLENS_DEVICE_EXTENSION ext = (PFLOWLENS_DEVICE_EXTENSION)DeviceObject->DeviceExtension;
    ext->capturing = FALSE;
    TRACE("Capture stopped");
    return STATUS_SUCCESS;
}

NTSTATUS FlowLensAddFilter(PDEVICE_OBJECT DeviceObject, PFLOWLENS_FILTER Filter) {
    PFLOWLENS_DEVICE_EXTENSION ext = (PFLOWLENS_DEVICE_EXTENSION)DeviceObject->DeviceExtension;
    if (ext->filter_count >= ext->filter_capacity) {
        ULONG new_cap = ext->filter_capacity == 0 ? 16 : ext->filter_capacity * 2;
        PFLOWLENS_FILTER nf = (PFLOWLENS_FILTER)ExAllocatePool2(
            POOL_FLAG_NON_PAGED, new_cap * sizeof(FLOWLENS_FILTER), 'LFwF');
        if (!nf) return STATUS_INSUFFICIENT_RESOURCES;
        if (ext->filters) {
            RtlCopyMemory(nf, ext->filters, ext->filter_count * sizeof(FLOWLENS_FILTER));
            ExFreePool(ext->filters);
        }
        ext->filters = nf;
        ext->filter_capacity = new_cap;
    }
    RtlCopyMemory(&ext->filters[ext->filter_count], Filter, sizeof(FLOWLENS_FILTER));
    ext->filter_count++;
    ext->active_filters = ext->filter_count;
    return STATUS_SUCCESS;
}

NTSTATUS FlowLensRemoveFilter(PDEVICE_OBJECT DeviceObject, ULONG Index) {
    PFLOWLENS_DEVICE_EXTENSION ext = (PFLOWLENS_DEVICE_EXTENSION)DeviceObject->DeviceExtension;
    if (Index >= ext->filter_count) return STATUS_INVALID_PARAMETER;
    if (Index < ext->filter_count - 1) {
        RtlCopyMemory(&ext->filters[Index], &ext->filters[Index + 1],
            (ext->filter_count - Index - 1) * sizeof(FLOWLENS_FILTER));
    }
    ext->filter_count--;
    ext->active_filters = ext->filter_count;
    return STATUS_SUCCESS;
}

NTSTATUS FlowLensReadEvents(PDEVICE_OBJECT DeviceObject, PUCHAR Output, ULONG OutputSize, PULONG BytesRead) {
    PFLOWLENS_DEVICE_EXTENSION ext = (PFLOWLENS_DEVICE_EXTENSION)DeviceObject->DeviceExtension;
    if (!ext->ring_buffer) { *BytesRead = 0; return STATUS_NO_MORE_ENTRIES; }
    return FlowLensRingBufferRead(ext->ring_buffer, Output, OutputSize, BytesRead);
}

NTSTATUS FlowLensGetStatus(PDEVICE_OBJECT DeviceObject, PFLOWLENS_STATUS Status) {
    PFLOWLENS_DEVICE_EXTENSION ext = (PFLOWLENS_DEVICE_EXTENSION)DeviceObject->DeviceExtension;
    Status->capturing = ext->capturing;
    Status->events_dropped = ext->events_dropped;
    Status->events_written = ext->events_written;
    Status->active_filters = ext->active_filters;
    LARGE_INTEGER now;
    KeQuerySystemTime(&now);
    Status->uptime_100ns = (ULONGLONG)(now.QuadPart - ext->start_time.QuadPart);
    return STATUS_SUCCESS;
}
NTSTATUS NTAPI FlowLensCreate(PDEVICE_OBJECT DeviceObject, PIRP Irp) {
    UNREFERENCED_PARAMETER(DeviceObject);
    Irp->IoStatus.Status = STATUS_SUCCESS;
    Irp->IoStatus.Information = 0;
    IoCompleteRequest(Irp, IO_NO_INCREMENT);
    return STATUS_SUCCESS;
}

NTSTATUS NTAPI FlowLensClose(PDEVICE_OBJECT DeviceObject, PIRP Irp) {
    UNREFERENCED_PARAMETER(DeviceObject);
    Irp->IoStatus.Status = STATUS_SUCCESS;
    Irp->IoStatus.Information = 0;
    IoCompleteRequest(Irp, IO_NO_INCREMENT);
    return STATUS_SUCCESS;
}

NTSTATUS NTAPI FlowLensDeviceControl(PDEVICE_OBJECT DeviceObject, PIRP Irp) {
    PIO_STACK_LOCATION stack = IoGetCurrentIrpStackLocation(Irp);
    ULONG code = stack->Parameters.DeviceIoControl.IoControlCode;
    ULONG in_len = stack->Parameters.DeviceIoControl.InputBufferLength;
    ULONG out_len = stack->Parameters.DeviceIoControl.OutputBufferLength;
    NTSTATUS status = STATUS_INVALID_DEVICE_REQUEST;
    ULONG info = 0;

    switch (code) {
        case FLOWLENS_IOCTL_START_CAPTURE:
            status = FlowLensStartCapture(DeviceObject);
            break;
        case FLOWLENS_IOCTL_STOP_CAPTURE:
            status = FlowLensStopCapture(DeviceObject);
            break;
        case FLOWLENS_IOCTL_ADD_FILTER:
            if (in_len >= sizeof(FLOWLENS_FILTER) && Irp->AssociatedIrp.SystemBuffer) {
                status = FlowLensAddFilter(DeviceObject, (PFLOWLENS_FILTER)Irp->AssociatedIrp.SystemBuffer);
            }
            break;
        case FLOWLENS_IOCTL_REMOVE_FILTER:
            if (in_len >= sizeof(ULONG) && Irp->AssociatedIrp.SystemBuffer) {
                status = FlowLensRemoveFilter(DeviceObject, *(PULONG)Irp->AssociatedIrp.SystemBuffer);
            }
            break;
        case FLOWLENS_IOCTL_READ_EVENTS:
            if (out_len > 0 && Irp->AssociatedIrp.SystemBuffer) {
                ULONG read = 0;
                status = FlowLensReadEvents(DeviceObject, (PUCHAR)Irp->AssociatedIrp.SystemBuffer, out_len, &read);
                info = read;
            }
            break;
        case FLOWLENS_IOCTL_GET_STATUS:
            if (out_len >= sizeof(FLOWLENS_STATUS) && Irp->AssociatedIrp.SystemBuffer) {
                status = FlowLensGetStatus(DeviceObject, (PFLOWLENS_STATUS)Irp->AssociatedIrp.SystemBuffer);
                info = sizeof(FLOWLENS_STATUS);
            }
            break;
        default:
            status = STATUS_INVALID_DEVICE_REQUEST;
            break;
    }

    Irp->IoStatus.Status = status;
    Irp->IoStatus.Information = info;
    IoCompleteRequest(Irp, IO_NO_INCREMENT);
    return status;
}

NTSTATUS NTAPI DriverEntry(PDRIVER_OBJECT DriverObject, PUNICODE_STRING RegistryPath) {
    UNREFERENCED_PARAMETER(RegistryPath);
    NTSTATUS status;
    PFLOWLENS_DEVICE_EXTENSION ext;

    DriverObject->DriverUnload = FlowLensUnload;
    DriverObject->MajorFunction[IRP_MJ_CREATE] = FlowLensCreate;
    DriverObject->MajorFunction[IRP_MJ_CLOSE] = FlowLensClose;
    DriverObject->MajorFunction[IRP_MJ_DEVICE_CONTROL] = FlowLensDeviceControl;

    status = IoCreateDevice(DriverObject, sizeof(FLOWLENS_DEVICE_EXTENSION),
        &g_DeviceName, FILE_DEVICE_UNKNOWN, FILE_DEVICE_SECURE_OPEN, FALSE, &g_DeviceObject);
    if (!NT_SUCCESS(status)) { TRACE("IoCreateDevice failed 0x%08x", status); return status; }

    g_DeviceObject->Flags |= DO_BUFFERED_IO;
    ext = (PFLOWLENS_DEVICE_EXTENSION)g_DeviceObject->DeviceExtension;
    RtlZeroMemory(ext, sizeof(FLOWLENS_DEVICE_EXTENSION));

    status = IoCreateSymbolicLink(&g_SymLinkName, &g_DeviceName);
    if (!NT_SUCCESS(status)) {
        IoDeleteDevice(g_DeviceObject);
        g_DeviceObject = NULL;
        return status;
    }

    status = FlowLensRegisterCallouts(g_DeviceObject);
    if (!NT_SUCCESS(status)) {
        TRACE("WFP register failed 0x%08x (non-fatal)", status);
    }

    g_DeviceObject->Flags &= ~DO_DEVICE_INITIALIZING;
    TRACE("DriverEntry complete");
    return STATUS_SUCCESS;
}

VOID NTAPI FlowLensUnload(PDRIVER_OBJECT DriverObject) {
    UNREFERENCED_PARAMETER(DriverObject);
    TRACE("Driver unload");
    if (g_DeviceObject) {
        FlowLensUnregisterCallouts(g_DeviceObject);
        PFLOWLENS_DEVICE_EXTENSION ext = (PFLOWLENS_DEVICE_EXTENSION)g_DeviceObject->DeviceExtension;
        if (ext->ring_buffer) FlowLensRingBufferFree(ext->ring_buffer);
        if (ext->filters) ExFreePool(ext->filters);
        IoDeleteSymbolicLink(&g_SymLinkName);
        IoDeleteDevice(g_DeviceObject);
        g_DeviceObject = NULL;
    }
}