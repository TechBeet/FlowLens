// FlowLens WFP Callout Driver Header
#pragma once

#include <ntddk.h>
#include <ndis.h>
#include <fwpsk.h>
#include <fwpmk.h>

#ifdef __cplusplus
extern "C" {
#endif

NTSTATUS NTAPI DriverEntry(PDRIVER_OBJECT DriverObject, PUNICODE_STRING RegistryPath);
VOID NTAPI FlowLensUnload(PDRIVER_OBJECT DriverObject);
NTSTATUS NTAPI FlowLensDeviceControl(PDEVICE_OBJECT DeviceObject, PIRP Irp);

// WFP Callout constants
#define FLOWLENS_CALLOUT_IP_V4_TRANSPORT_ID 0x1001
#define FLOWLENS_CALLOUT_IP_V6_TRANSPORT_ID 0x1002
#define FLOWLENS_CALLOUT_ALE_AUTH_CONNECT_V4_ID 0x1003
#define FLOWLENS_CALLOUT_ALE_AUTH_CONNECT_V6_ID 0x1004

// IOCTL codes for user-mode communication
#define FLOWLENS_IOCTL_START_CAPTURE CTL_CODE(FILE_DEVICE_UNKNOWN, 0x800, METHOD_BUFFERED, FILE_ANY_ACCESS)
#define FLOWLENS_IOCTL_STOP_CAPTURE CTL_CODE(FILE_DEVICE_UNKNOWN, 0x801, METHOD_BUFFERED, FILE_ANY_ACCESS)
#define FLOWLENS_IOCTL_ADD_FILTER CTL_CODE(FILE_DEVICE_UNKNOWN, 0x802, METHOD_BUFFERED, FILE_ANY_ACCESS)
#define FLOWLENS_IOCTL_REMOVE_FILTER CTL_CODE(FILE_DEVICE_UNKNOWN, 0x803, METHOD_BUFFERED, FILE_ANY_ACCESS)
#define FLOWLENS_IOCTL_READ_EVENTS   CTL_CODE(FILE_DEVICE_UNKNOWN, 0x804, METHOD_BUFFERED, FILE_ANY_ACCESS)
#define FLOWLENS_IOCTL_GET_STATUS    CTL_CODE(FILE_DEVICE_UNKNOWN, 0x805, METHOD_BUFFERED, FILE_ANY_ACCESS)

// Ring buffer for packet data
#define RING_BUFFER_SIZE (16 * 1024 * 1024)  // 16MB ring buffer

// IOCTL codes
// ---------------------------------------------------------------------------
typedef enum _FLOWLENS_DIRECTION {
    FLOWLENS_DIR_INBOUND = 0,
    FLOWLENS_DIR_OUTBOUND = 1,
} FLOWLENS_DIRECTION;

// ---------------------------------------------------------------------------
// Traffic protocol
// ---------------------------------------------------------------------------
typedef enum _FLOWLENS_PROTOCOL {
    FLOWLENS_PROTO_TCP = 6,
    FLOWLENS_PROTO_UDP = 17,
    FLOWLENS_PROTO_ICMP = 1,
    FLOWLENS_PROTO_OTHER = 0,
} FLOWLENS_PROTOCOL;

// ---------------------------------------------------------------------------
// Traffic event (kernel -> user mode)
// ---------------------------------------------------------------------------
typedef struct _FLOWLENS_TRAFFIC_EVENT {
    ULONGLONG timestamp;
    ULONG     pid;
    CHAR      process_name[260];
    ULONG     src_addr;
    USHORT    src_port;
    ULONG     dst_addr;
    USHORT    dst_port;
    UCHAR     protocol;
    UCHAR     direction;
    ULONG     data_size;
} FLOWLENS_TRAFFIC_EVENT, *PFLOWLENS_TRAFFIC_EVENT;

// ---------------------------------------------------------------------------
// Filter rule (user -> kernel)
// ---------------------------------------------------------------------------
typedef struct _FLOWLENS_FILTER {
    ULONG src_addr;
    ULONG src_mask;
    ULONG dst_addr;
    ULONG dst_mask;
    USHORT src_port;
    USHORT dst_port;
    UCHAR  protocol;
    UCHAR  direction;
    BOOLEAN enabled;
} FLOWLENS_FILTER, *PFLOWLENS_FILTER;

// ---------------------------------------------------------------------------
// Driver status
// ---------------------------------------------------------------------------
typedef struct _FLOWLENS_STATUS {
    BOOLEAN capturing;
    ULONG   events_dropped;
    ULONG   events_written;
    ULONG   active_filters;
    ULONGLONG uptime_100ns;
} FLOWLENS_STATUS, *PFLOWLENS_STATUS;

#define MAX_EVENTS_PER_READ 64

// ---------------------------------------------------------------------------
// Function declarations
// ---------------------------------------------------------------------------
NTSTATUS NTAPI FlowLensCreate(PDEVICE_OBJECT DeviceObject, PIRP Irp);
NTSTATUS NTAPI FlowLensClose(PDEVICE_OBJECT DeviceObject, PIRP Irp);

VOID NTAPI FlowLensClassify(
    _In_ const FWPS_INCOMING_VALUES *inFixedValues,
    _In_ const FWPS_INCOMING_METADATA_VALUES *inMetaValues,
    _Inout_ void *layerData,
    _In_ const void *classifyContext,
    _In_ const FWPS_FILTER *filter,
    _In_ UINT64 flowContext,
    _Inout_ FWPS_CLASSIFY_OUT *classifyOut
);

NTSTATUS NTAPI FlowLensNotify(
    _In_ FWPS_CALLOUT_NOTIFY_TYPE notifyType,
    _In_ const GUID *filterKey,
    _In_ const FWPS_FILTER *filter
);

VOID NTAPI FlowLensDelete(_In_ UINT16 layerId, _In_ UINT32 calloutId);

NTSTATUS FlowLensRegisterCallouts(PDEVICE_OBJECT DeviceObject);
VOID FlowLensUnregisterCallouts(PDEVICE_OBJECT DeviceObject);

NTSTATUS FlowLensStartCapture(PDEVICE_OBJECT DeviceObject);
NTSTATUS FlowLensStopCapture(PDEVICE_OBJECT DeviceObject);

NTSTATUS FlowLensAddFilter(PDEVICE_OBJECT DeviceObject, PFLOWLENS_FILTER Filter);
NTSTATUS FlowLensRemoveFilter(PDEVICE_OBJECT DeviceObject, ULONG Index);

NTSTATUS FlowLensReadEvents(PDEVICE_OBJECT DeviceObject, PUCHAR Output, ULONG OutputSize, PULONG BytesRead);
NTSTATUS FlowLensGetStatus(PDEVICE_OBJECT DeviceObject, PFLOWLENS_STATUS Status);

#ifdef __cplusplus
}
#endif