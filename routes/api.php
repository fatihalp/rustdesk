<?php

use App\Http\Controllers\Api\BuildRunnerController;
use App\Http\Controllers\Api\DeviceSyncController;
use App\Http\Middleware\AuthenticateBuildRunner;
use Illuminate\Support\Facades\Route;

// Consumed by the RustDesk client (client/src/hbbs_http/sync.rs).
Route::post('heartbeat', [DeviceSyncController::class, 'heartbeat'])->middleware('throttle:120,1');
Route::post('sysinfo', [DeviceSyncController::class, 'sysinfo'])->middleware('throttle:30,1');
Route::post('sysinfo_ver', [DeviceSyncController::class, 'sysinfoVersion'])->middleware('throttle:120,1');

// Consumed by .github/workflows/brand-package.yml.
Route::middleware(AuthenticateBuildRunner::class)->prefix('builds/{build}')->group(function () {
    Route::get('manifest', [BuildRunnerController::class, 'manifest']);
    Route::post('status', [BuildRunnerController::class, 'status']);
    Route::post('artifact', [BuildRunnerController::class, 'artifact']);
});
