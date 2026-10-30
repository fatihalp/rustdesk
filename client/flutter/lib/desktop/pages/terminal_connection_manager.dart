import 'package:flutter/foundation.dart';
import 'package:get/get.dart';
import '../../models/model.dart';

class TerminalConnectionManager {
  static final Map<String, FFI> _connections = {};
  static final Map<String, int> _connectionRefCount = {};
  
  static final Map<String, String> _serviceIds = {};

  static FFI getConnection({
    required String peerId,
    required String? password,
    required bool? isSharedPassword,
    required bool? forceRelay,
    required String? connToken,
  }) {
    final existingFfi = _connections[peerId];
    if (existingFfi != null && !existingFfi.closed) {
      _connectionRefCount[peerId] = (_connectionRefCount[peerId] ?? 0) + 1;
      debugPrint('[TerminalConnectionManager] Reusing existing connection for peer $peerId. Reference count: ${_connectionRefCount[peerId]}');
      return existingFfi;
    }

    debugPrint('[TerminalConnectionManager] Creating new terminal connection for peer $peerId');
    final ffi = FFI(null);
    ffi.start(
      peerId,
      password: password,
      isSharedPassword: isSharedPassword,
      forceRelay: forceRelay,
      connToken: connToken,
      isTerminal: true,
    );
    
    _connections[peerId] = ffi;
    _connectionRefCount[peerId] = 1;
    
    Get.put<FFI>(ffi, tag: 'terminal_$peerId');
    
    debugPrint('[TerminalConnectionManager] New connection created. Total connections: ${_connections.length}');
    return ffi;
  }

  static void releaseConnection(String peerId) {
    final refCount = _connectionRefCount[peerId] ?? 0;
    debugPrint('[TerminalConnectionManager] Releasing connection for peer $peerId. Current ref count: $refCount');
    
    if (refCount <= 1) {
      final ffi = _connections[peerId];
      if (ffi != null) {
        debugPrint('[TerminalConnectionManager] Closing connection for peer $peerId (last reference)');
        ffi.close();
        _connections.remove(peerId);
        _connectionRefCount.remove(peerId);
        Get.delete<FFI>(tag: 'terminal_$peerId');
      }
    } else {
      _connectionRefCount[peerId] = refCount - 1;
      debugPrint('[TerminalConnectionManager] Connection still in use. New ref count: ${_connectionRefCount[peerId]}');
    }
  }

  static bool hasConnection(String peerId) {
    final ffi = _connections[peerId];
    return ffi != null && !ffi.closed;
  }
  
  static FFI? getExistingConnection(String peerId) {
    return _connections[peerId];
  }

  static int getConnectionCount() => _connections.length;
  
  static int getTerminalCount(String peerId) => _connectionRefCount[peerId] ?? 0;
  
  static String? getServiceId(String peerId) => _serviceIds[peerId];
  
  static void setServiceId(String peerId, String serviceId) {
    _serviceIds[peerId] = serviceId;
    debugPrint('[TerminalConnectionManager] Service ID for $peerId: $serviceId');
  }
}