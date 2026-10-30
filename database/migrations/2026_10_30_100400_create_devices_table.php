<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::create('devices', function (Blueprint $table) {
            $table->id();
            $table->foreignId('client_app_id')->nullable()->constrained()->nullOnDelete();
            $table->string('supportdesk_id')->unique();
            $table->string('uuid')->nullable();
            $table->string('hostname')->nullable();
            $table->string('username')->nullable();
            $table->string('os')->nullable();
            $table->string('platform', 16)->nullable();
            $table->string('cpu')->nullable();
            $table->string('memory')->nullable();
            $table->string('client_version')->nullable();
            $table->string('ip_address', 45)->nullable();
            $table->text('unattended_password')->nullable();
            $table->unsignedInteger('active_connections')->default(0);
            $table->text('notes')->nullable();
            $table->timestamp('first_seen_at')->nullable();
            $table->timestamp('last_seen_at')->nullable();
            $table->timestamps();

            $table->index('last_seen_at');
        });
    }

    public function down(): void
    {
        Schema::dropIfExists('devices');
    }
};
