// FlowLens WFP Callout Driver - C/C++ implementation
// This is the kernel-mode driver that enables proxyless packet capture
// on Windows 10/11 without requiring Npcap or WinPcap.

#include "flowlens_driver.h"

#include <ntddk.h>
#include <ndis.h>
#include <fwpsk.h>

// Driver entry
EXTERN_C NTSTATUS NTAPI DriverEntry(
    _In_ PDRIVER_OBJECT DriverObject,
    _In_ PUNICODE_STRING RegistryPath
) {
    DbgPrintEx(DPFLTR_IHDPDX_ID, DPFLTR_ERROR_LEVEL,
        "FlowLens: DriverEntry called\n");

    DriverObject->DriverUnload = FlowLensUnload;
    DriverObject->MajorFunction[IRP_MJ_DEVICE_CONTROL] = FlowLensDeviceControl;

    return STATUS_SUCCESS;
}

EXTERN_C VOID NTAPI FlowLensUnload(
    _In_ PDRIVER_OBJECT DriverObject
) {
    DbgPrintEx(DPFLTR_IHDPDX_ID, DPFLTR_ERROR_LEVEL,
        "FlowLens: Driver unload\n");
}

// IOCTL handler
EXTERN_C NTSTATUS NTAPI FlowLensDeviceControl(
    _In_ PDEVICE_OBJECT DeviceObject,
    _In_ PIRP Irp
) {
    DbgPrintEx(DPFLTR_IHDPDX_ID, DPFLTR_ERROR_LEVEL,
        "FlowLens: DeviceControl\n");
    return STATUS_SUCCESS;
}