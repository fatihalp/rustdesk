<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::create('download_links', function (Blueprint $table) {
            $table->id();
            $table->foreignId('client_app_id')->constrained()->cascadeOnDelete();
            $table->string('token', 64)->unique();
            $table->string('label')->nullable();
            $table->timestamp('expires_at')->nullable();
            $table->unsignedInteger('max_downloads')->nullable();
            $table->unsignedInteger('download_count')->default(0);
            $table->timestamp('last_downloaded_at')->nullable();
            $table->timestamp('revoked_at')->nullable();
            $table->timestamps();
        });
    }

    public function down(): void
    {
        Schema::dropIfExists('download_links');
    }
};
