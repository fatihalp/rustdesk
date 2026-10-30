<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::create('client_apps', function (Blueprint $table) {
            $table->id();
            $table->foreignId('customer_id')->constrained()->cascadeOnDelete();
            $table->string('app_name');
            $table->string('slug')->unique();
            $table->string('logo_path')->nullable();
            $table->string('enroll_token', 64)->unique();
            $table->boolean('is_active')->default(true);
            $table->boolean('unattended')->default(true);
            $table->text('notes')->nullable();
            $table->timestamps();
        });
    }

    public function down(): void
    {
        Schema::dropIfExists('client_apps');
    }
};
