#include "pch.h"

#include <Windows.h>
#include <winspool.h>
#include <setupapi.h>
#include <strutil.h>
#include <memory>
#include <string>
#include <functional>
#include <vector>
#include <iostream>

#include "Common.h"

#pragma comment(lib, "setupapi.lib")
#pragma comment(lib, "winspool.lib")

namespace RemotePrinter
{
#define HRESULT_ERR_ELEMENT_NOT_FOUND 0x80070490

    LPCWCH RD_DRIVER_INF_PATH = L"drivers\\RustDeskPrinterDriver\\RustDeskPrinterDriver.inf";
    LPCWCH RD_PRINTER_DRIVER_NAME = L"SupportDesk v4 Printer Driver";
    LPCWCH RD_DEFAULT_APP_NAME = L"SupportDesk";
    LPCWCH XCV_MONITOR_LOCAL_PORT = L",XcvMonitor Local Port";

    static std::wstring printerNameOf(const std::wstring &appName)
    {
        return (appName.empty() ? std::wstring(RD_DEFAULT_APP_NAME) : appName) + L" Printer";
    }

    using FuncEnum = std::function<BOOL(DWORD level, LPBYTE pDriverInfo, DWORD cbBuf, LPDWORD pcbNeeded, LPDWORD pcReturned)>;
    template <typename T, typename R>
    using FuncOnData = std::function<std::shared_ptr<R>(const T &)>;
    template <typename R>
    using FuncOnNoData = std::function<std::shared_ptr<R>()>;

    template <class T, class R>
    std::shared_ptr<R> commonEnum(std::wstring funcName, FuncEnum func, DWORD level, FuncOnData<T, R> onData, FuncOnNoData<R> onNoData)
    {
        DWORD needed = 0;
        DWORD returned = 0;
        func(level, NULL, 0, &needed, &returned);
        if (needed == 0)
        {
            return onNoData();
        }

        std::vector<BYTE> buffer(needed);
        if (!func(level, buffer.data(), needed, &needed, &returned))
        {
            return nullptr;
        }

        T *pPortInfo = reinterpret_cast<T *>(buffer.data());
        for (DWORD i = 0; i < returned; i++)
        {
            auto r = onData(pPortInfo[i]);
            if (r)
            {
                return r;
            }
        }
        return onNoData();
    }

    BOOL isNameEqual(LPCWSTR lhs, LPCWSTR rhs)
    {
        return lstrcmpiW(lhs, rhs) == 0 ? TRUE : FALSE;
    }

    BOOL enumPrinterPort(
        DWORD level,
        LPBYTE pPortInfo,
        DWORD cbBuf,
        LPDWORD pcbNeeded,
        LPDWORD pcReturned)
    {
        return EnumPortsW(NULL, level, pPortInfo, cbBuf, pcbNeeded, pcReturned);
    }

    BOOL isPortExists(LPCWSTR port)
    {
        auto onData = [port](const PORT_INFO_2 &info)
        {
            if (isNameEqual(info.pPortName, port) == TRUE) {
                return std::shared_ptr<BOOL>(new BOOL(TRUE));
            }
            else {
                return std::shared_ptr<BOOL>(nullptr);
            } };
        auto onNoData = []()
        { return nullptr; };
        auto res = commonEnum<PORT_INFO_2, BOOL>(L"EnumPortsW", enumPrinterPort, 2, onData, onNoData);
        if (res == nullptr)
        {
            return false;
        }
        else
        {
            return *res;
        }
    }

    BOOL executeOnLocalPort(LPCWSTR port, LPCWSTR command)
    {
        PRINTER_DEFAULTSW dft = {0};
        dft.DesiredAccess = SERVER_WRITE;
        HANDLE hMonitor = NULL;
        if (OpenPrinterW(const_cast<LPWSTR>(XCV_MONITOR_LOCAL_PORT), &hMonitor, &dft) == FALSE)
        {
            return FALSE;
        }

        DWORD outputNeeded = 0;
        DWORD status = 0;
        if (XcvDataW(hMonitor, command, (LPBYTE)port, (lstrlenW(port) + 1) * 2, NULL, 0, &outputNeeded, &status) == FALSE)
        {
            ClosePrinter(hMonitor);
            return FALSE;
        }

        ClosePrinter(hMonitor);
        return TRUE;
    }

    BOOL addLocalPort(LPCWSTR port)
    {
        return executeOnLocalPort(port, L"AddPort");
    }

    BOOL deleteLocalPort(LPCWSTR port)
    {
        return executeOnLocalPort(port, L"DeletePort");
    }

    BOOL checkAddLocalPort(LPCWSTR port)
    {
        if (!isPortExists(port))
        {
            return addLocalPort(port);
        }
        return TRUE;
    }

    std::wstring getPrinterInstalledOnPort(LPCWSTR port);

    BOOL checkDeleteLocalPort(LPCWSTR port)
    {
        if (isPortExists(port))
        {
            if (getPrinterInstalledOnPort(port) != L"")
            {
                WcaLog(LOGMSG_STANDARD, "The printer is installed on the port. Please remove the printer first.\n");
                return FALSE;
            }
            return deleteLocalPort(port);
        }
        return TRUE;
    }

    BOOL enumPrinterDriver(
        DWORD level,
        LPBYTE pDriverInfo,
        DWORD cbBuf,
        LPDWORD pcbNeeded,
        LPDWORD pcReturned)
    {
        return EnumPrinterDriversW(
            NULL,
            NULL,
            level,
            pDriverInfo,
            cbBuf,
            pcbNeeded,
            pcReturned);
    }

    DWORDLONG getInstalledDriverVersion(LPCWSTR name)
    {
        auto onData = [name](const DRIVER_INFO_6W &info)
        {
            if (isNameEqual(name, info.pName) == TRUE)
            {
                return std::shared_ptr<DWORDLONG>(new DWORDLONG(info.dwlDriverVersion));
            }
            else
            {
                return std::shared_ptr<DWORDLONG>(nullptr);
            } };
        auto onNoData = []()
        { return nullptr; };
        auto res = commonEnum<DRIVER_INFO_6W, DWORDLONG>(L"EnumPrinterDriversW", enumPrinterDriver, 6, onData, onNoData);
        if (res == nullptr)
        {
            return 0;
        }
        else
        {
            return *res;
        }
    }

    std::wstring findInf(LPCWSTR name)
    {
        auto onData = [name](const DRIVER_INFO_8W &info)
        {
            if (isNameEqual(name, info.pName) == TRUE)
            {
                return std::shared_ptr<std::wstring>(new std::wstring(info.pszInfPath));
            }
            else
            {
                return std::shared_ptr<std::wstring>(nullptr);
            } };
        auto onNoData = []()
        { return nullptr; };
        auto res = commonEnum<DRIVER_INFO_8W, std::wstring>(L"EnumPrinterDriversW", enumPrinterDriver, 8, onData, onNoData);
        if (res == nullptr)
        {
            return L"";
        }
        else
        {
            return *res;
        }
    }

    BOOL deletePrinterDriver(LPCWSTR name)
    {
        BOOL res = DeletePrinterDriverExW(NULL, NULL, const_cast<LPWSTR>(name), DPD_DELETE_ALL_FILES, 0);
        if (res == FALSE)
        {
            DWORD error = GetLastError();
            if (error == ERROR_UNKNOWN_PRINTER_DRIVER)
            {
                return TRUE;
            }
            else
            {
                WcaLog(LOGMSG_STANDARD, "Failed to delete printer driver. Error (%d)\n", error);
            }
        }
        return res;
    }

    BOOL deletePrinterDriverPackage(const std::wstring &inf)
    {
        int tries = 3;
        HRESULT result = S_FALSE;
        while ((result = DeletePrinterDriverPackage(NULL, inf.c_str(), NULL)) != S_OK)
        {
            if (result == HRESULT_ERR_ELEMENT_NOT_FOUND)
            {
                return TRUE;
            }

            WcaLog(LOGMSG_STANDARD, "Failed to delete printer driver package. HRESULT (%d)\n", result);
            tries--;
            if (tries <= 0)
            {
                return FALSE;
            }
            Sleep(2000);
        }
        return S_OK;
    }

    BOOL uninstallDriver(LPCWSTR name)
    {
        auto infFile = findInf(name);
        if (!deletePrinterDriver(name))
        {
            return FALSE;
        }
        if (infFile != L"" && !deletePrinterDriverPackage(infFile))
        {
            return FALSE;
        }
        return TRUE;
    }

    BOOL installDriver(LPCWSTR name, LPCWSTR inf)
    {
        DWORD size = MAX_PATH * 10;
        wchar_t package_path[MAX_PATH * 10] = {0};
        HRESULT result = UploadPrinterDriverPackage(
            NULL, inf, NULL,
            UPDP_SILENT_UPLOAD | UPDP_UPLOAD_ALWAYS, NULL, package_path, &size);
        if (result != S_OK)
        {
            WcaLog(LOGMSG_STANDARD, "Uploading the printer driver package to the driver cache silently, failed. Will retry with user UI. HRESULT (%d)\n", result);
            result = UploadPrinterDriverPackage(
                NULL, inf, NULL, UPDP_UPLOAD_ALWAYS,
                GetForegroundWindow(), package_path, &size);
            if (result != S_OK)
            {
                WcaLog(LOGMSG_STANDARD, "Uploading the printer driver package to the driver cache failed with user UI. Aborting...\n");
                return FALSE;
            }
        }

        result = InstallPrinterDriverFromPackage(
            NULL, package_path, name, NULL, IPDFP_COPY_ALL_FILES);
        if (result != S_OK)
        {
            WcaLog(LOGMSG_STANDARD, "Installing the printer driver failed. HRESULT (%d)\n", result);
        }
        return result == S_OK;
    }

    BOOL enumLocalPrinter(
        DWORD level,
        LPBYTE pPrinterInfo,
        DWORD cbBuf,
        LPDWORD pcbNeeded,
        LPDWORD pcReturned)
    {
        return EnumPrintersW(PRINTER_ENUM_LOCAL, NULL, level, pPrinterInfo, cbBuf, pcbNeeded, pcReturned);
    }

    static HRESULT queryPrinterPresence(LPCWSTR name)
    {
        constexpr DWORD printerInfoLevel = 1;
        DWORD needed = 0;
        DWORD returned = 0;
        if (!enumLocalPrinter(printerInfoLevel, NULL, 0, &needed, &returned))
        {
            const DWORD error = GetLastError();
            if (error != ERROR_INSUFFICIENT_BUFFER || needed == 0)
            {
                return error == ERROR_SUCCESS ? E_FAIL : HRESULT_FROM_WIN32(error);
            }
        }
        if (needed == 0)
        {
            return S_FALSE;
        }

        std::vector<BYTE> buffer(needed);
        if (!enumLocalPrinter(printerInfoLevel, buffer.data(), needed, &needed, &returned))
        {
            const DWORD error = GetLastError();
            return error == ERROR_SUCCESS ? E_FAIL : HRESULT_FROM_WIN32(error);
        }

        const auto printers = reinterpret_cast<const PRINTER_INFO_1W *>(buffer.data());
        for (DWORD i = 0; i < returned; i++)
        {
            if (isNameEqual(name, printers[i].pName))
            {
                return S_OK;
            }
        }
        return S_FALSE;
    }

    BOOL isPrinterAdded(LPCWSTR name)
    {
        auto onData = [name](const PRINTER_INFO_1W &info)
        {
            if (isNameEqual(name, info.pName) == TRUE)
            {
                return std::shared_ptr<BOOL>(new BOOL(TRUE));
            }
            else
            {
                return std::shared_ptr<BOOL>(nullptr);
            } };
        auto onNoData = []()
        { return nullptr; };
        auto res = commonEnum<PRINTER_INFO_1W, BOOL>(L"EnumPrintersW", enumLocalPrinter, 1, onData, onNoData);
        if (res == nullptr)
        {
            return FALSE;
        }
        else
        {
            return *res;
        }
    }

    std::wstring getPrinterInstalledOnPort(LPCWSTR port)
    {
        auto onData = [port](const PRINTER_INFO_2W &info)
        {
            if (isNameEqual(port, info.pPortName) == TRUE)
            {
                return std::shared_ptr<std::wstring>(new std::wstring(info.pPrinterName));
            }
            else
            {
                return std::shared_ptr<std::wstring>(nullptr);
            } };
        auto onNoData = []()
        { return nullptr; };
        auto res = commonEnum<PRINTER_INFO_2W, std::wstring>(L"EnumPrintersW", enumLocalPrinter, 2, onData, onNoData);
        if (res == nullptr)
        {
            return L"";
        }
        else
        {
            return *res;
        }
    }

    BOOL addPrinter(LPCWSTR name, LPCWSTR driver, LPCWSTR port)
    {
        PRINTER_INFO_2W printerInfo = {0};
        printerInfo.pPrinterName = const_cast<LPWSTR>(name);
        printerInfo.pPortName = const_cast<LPWSTR>(port);
        printerInfo.pDriverName = const_cast<LPWSTR>(driver);
        printerInfo.pPrintProcessor = const_cast<LPWSTR>(L"WinPrint");
        printerInfo.pDatatype = const_cast<LPWSTR>(L"RAW");
        printerInfo.Attributes = PRINTER_ATTRIBUTE_LOCAL;
        HANDLE hPrinter = AddPrinterW(NULL, 2, (LPBYTE)&printerInfo);
        return hPrinter == NULL ? FALSE : TRUE;
    }

    VOID deletePrinter(LPCWSTR name)
    {
        PRINTER_DEFAULTSW dft = {0};
        dft.DesiredAccess = PRINTER_ALL_ACCESS;
        HANDLE hPrinter = NULL;
        if (OpenPrinterW(const_cast<LPWSTR>(name), &hPrinter, &dft) == FALSE)
        {
            DWORD error = GetLastError();
            if (error == ERROR_INVALID_PRINTER_NAME)
            {
                return;
            }
            WcaLog(LOGMSG_STANDARD, "Failed to open printer. error (%d)\n", error);
            return;
        }

        if (SetPrinterW(hPrinter, 0, NULL, PRINTER_CONTROL_PURGE) == FALSE)
        {
            ClosePrinter(hPrinter);
            WcaLog(LOGMSG_STANDARD, "Failed to purge printer queue. error (%d)\n", GetLastError());
            return;
        }

        if (DeletePrinter(hPrinter) == FALSE)
        {
            ClosePrinter(hPrinter);
            WcaLog(LOGMSG_STANDARD, "Failed to delete printer. error (%d)\n", GetLastError());
            return;
        }

        ClosePrinter(hPrinter);
    }

    bool FileExists(const std::wstring &filePath)
    {
        DWORD fileAttributes = GetFileAttributes(filePath.c_str());
        return (fileAttributes != INVALID_FILE_ATTRIBUTES && !(fileAttributes & FILE_ATTRIBUTE_DIRECTORY));
    }

    VOID installUpdatePrinter(const std::wstring &installFolder, const std::wstring &appName)
    {
        const std::wstring printerName = printerNameOf(appName);
        const LPCWCH RD_PRINTER_NAME = printerName.c_str();
        const LPCWCH RD_PRINTER_PORT = printerName.c_str();

        const std::wstring infFile = installFolder + L"\\" + RemotePrinter::RD_DRIVER_INF_PATH;
        if (!FileExists(infFile))
        {
            WcaLog(LOGMSG_STANDARD, "Printer driver INF file not found, aborting...\n");
            return;
        }

        if (!checkAddLocalPort(RD_PRINTER_PORT))
        {
            WcaLog(LOGMSG_STANDARD, "Failed to check add local port, error (%d)\n", GetLastError());
            return;
        }
        else
        {
            WcaLog(LOGMSG_STANDARD, "Local port added successfully\n");
        }

        if (getInstalledDriverVersion(RD_PRINTER_DRIVER_NAME) > 0)
        {
            deletePrinter(RD_PRINTER_NAME);
            if (FALSE == uninstallDriver(RD_PRINTER_DRIVER_NAME))
            {
                WcaLog(LOGMSG_STANDARD, "Failed to uninstall previous printer driver, error (%d)\n", GetLastError());
            }
        }

        if (FALSE == installDriver(RD_PRINTER_DRIVER_NAME, infFile.c_str()))
        {
            WcaLog(LOGMSG_STANDARD, "Driver installation failed, still try to add the printer\n");
        }
        else
        {
            WcaLog(LOGMSG_STANDARD, "Driver installed successfully\n");
        }

        if (FALSE == addPrinter(RD_PRINTER_NAME, RD_PRINTER_DRIVER_NAME, RD_PRINTER_PORT))
        {
            WcaLog(LOGMSG_STANDARD, "Failed to add printer, error (%d)\n", GetLastError());
        }
        else
        {
            WcaLog(LOGMSG_STANDARD, "Printer installed successfully\n");
        }
    }

    VOID uninstallPrinter(const std::wstring &appName)
    {
        const std::wstring printerName = printerNameOf(appName);

        deletePrinter(printerName.c_str());
        WcaLog(LOGMSG_STANDARD, "Deleted the printer\n");
        uninstallDriver(RD_PRINTER_DRIVER_NAME);
        WcaLog(LOGMSG_STANDARD, "Uninstalled the printer driver\n");
        checkDeleteLocalPort(printerName.c_str());
        WcaLog(LOGMSG_STANDARD, "Deleted the local port\n");
    }
}

extern "C" UINT __stdcall SetPrinterInstallDefault(__in MSIHANDLE hInstall)
{
    HRESULT hr = S_OK;
    LPWSTR appName = NULL;

    hr = WcaInitialize(hInstall, "SetPrinterInstallDefault");
    ExitOnFailure(hr, "Failed to initialize printer detection");

    hr = WcaGetProperty(L"ProductName", &appName);
    ExitOnFailure(hr, "Failed to get the printer's application name");

    hr = RemotePrinter::queryPrinterPresence(RemotePrinter::printerNameOf(appName).c_str());
    if (FAILED(hr))
    {
        WcaLog(LOGMSG_STANDARD, "Failed to detect the existing printer (0x%08X); skipping printer installation.", static_cast<UINT>(hr));
    }

    if (hr != S_OK)
    {
        hr = WcaSetProperty(L"INSTALLPRINTER", L"0");
        ExitOnFailure(hr, "Failed to disable printer installation");
        WcaLog(LOGMSG_STANDARD, "The application's printer was not confirmed present; leaving printer installation disabled by default.");
    }

LExit:
    ReleaseStr(appName);
    return WcaFinalize(SUCCEEDED(hr) ? ERROR_SUCCESS : ERROR_INSTALL_FAILURE);
}
