#pragma once
#include <Windows.h>
#include <tchar.h>
#include <swdevice.h>

#ifdef __cplusplus
extern "C" {
#endif

BOOL InstallUpdate(LPCTSTR fullInfPath, PBOOL rebootRequired);

BOOL Uninstall(LPCTSTR fullInfPath, PBOOL rebootRequired);

BOOL IsDeviceCreated(PBOOL created);

BOOL DeviceCreate(PHSWDEVICE hSwDevice);

BOOL DeviceCreateWithLifetime(SW_DEVICE_LIFETIME * lifetime, PHSWDEVICE hSwDevice);

VOID DeviceClose(HSWDEVICE hSwDevice);

BOOL MonitorPlugIn(UINT index, UINT edid, INT retries);

BOOL MonitorPlugOut(UINT index);

typedef struct _MonitorMode {
    DWORD width;
    DWORD height;
    DWORD sync;
} MonitorMode, *PMonitorMode;

BOOL MonitorModesUpdate(UINT index, UINT modeCount, PMonitorMode modes);

const char* GetLastMsg();

VOID SetPrintErrMsg(BOOL b);

#ifdef __cplusplus
}
#endif
