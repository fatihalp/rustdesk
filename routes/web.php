<?php

use App\Http\Controllers\DownloadController;
use Illuminate\Support\Facades\Route;

Route::redirect('/', '/admin');

Route::get('/d/{token}', [DownloadController::class, 'show'])->name('download.show');
Route::get('/d/{token}/{platform}', [DownloadController::class, 'download'])
    ->middleware('throttle:30,1')
    ->name('download.file');
