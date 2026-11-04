<?php

use App\Http\Controllers\Api\Portal\AuthController;
use App\Http\Controllers\Api\Portal\BrandController;
use App\Http\Controllers\Api\Portal\BuildController;
use App\Http\Controllers\Api\Portal\DeviceController;
use App\Http\Controllers\Api\Portal\LinkController;
use App\Http\Controllers\Api\Portal\OAuthController;
use App\Http\Controllers\DownloadController;
use App\Http\Middleware\EnsureAdmin;
use App\Http\Middleware\EnsureTenant;
use Illuminate\Support\Facades\Route;

Route::redirect('/', '/portal');

Route::get('/s/{slug}', [DownloadController::class, 'short'])->name('shortlink.show');
Route::get('/d/{token}', [DownloadController::class, 'show'])->name('download.show');
Route::get('/d/{token}/{platform}', [DownloadController::class, 'download'])
    ->middleware('throttle:30,1')
    ->name('download.file');

Route::get('/admin/builds/{build}/download', [DownloadController::class, 'adminDownload'])
    ->middleware(['auth', EnsureAdmin::class])
    ->name('admin.build.download');

Route::get('/auth/{provider}/redirect', [OAuthController::class, 'redirect'])->name('oauth.redirect');
Route::get('/auth/{provider}/callback', [OAuthController::class, 'callback'])->name('oauth.callback');

Route::prefix('api/portal')->group(function () {
    Route::middleware('throttle:10,1')->group(function () {
        Route::post('auth/login', [AuthController::class, 'login']);
        Route::post('auth/register', [AuthController::class, 'register']);
    });
    Route::get('auth/me', [AuthController::class, 'me']);
    Route::post('auth/logout', [AuthController::class, 'logout']);

    Route::middleware(['auth', EnsureTenant::class])->group(function () {
        Route::get('brand', [BrandController::class, 'show']);
        Route::post('brand', [BrandController::class, 'update']);
        Route::get('brand/manifest', [BrandController::class, 'manifest']);

        Route::get('builds', [BuildController::class, 'index']);
        Route::post('builds/trigger', [BuildController::class, 'trigger'])->middleware('throttle:6,1');

        Route::get('links', [LinkController::class, 'index']);
        Route::post('links', [LinkController::class, 'store']);
        Route::delete('links/{link}', [LinkController::class, 'destroy']);

        Route::get('devices', [DeviceController::class, 'index']);
        Route::put('devices/{device}', [DeviceController::class, 'update']);
        Route::post('devices/{device}/action', [DeviceController::class, 'action']);
        Route::get('devices/{device}/connect', [DeviceController::class, 'connectInfo']);
        Route::delete('devices/{device}', [DeviceController::class, 'destroy']);
    });
});

Route::view('/portal/{any?}', 'portal')->where('any', '.*')->name('portal');
