<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::create('client_builds', function (Blueprint $table) {
            $table->id();
            $table->foreignId('client_app_id')->constrained()->cascadeOnDelete();
            $table->string('platform', 16);
            $table->string('status', 16)->default('pending');
            $table->string('version')->nullable();
            $table->string('github_run_url')->nullable();
            $table->string('artifact_path')->nullable();
            $table->string('artifact_name')->nullable();
            $table->string('sha256', 64)->nullable();
            $table->unsignedBigInteger('size_bytes')->nullable();
            $table->boolean('signed')->default(false);
            $table->text('error')->nullable();
            $table->timestamp('started_at')->nullable();
            $table->timestamp('finished_at')->nullable();
            $table->timestamps();

            $table->index(['client_app_id', 'platform', 'status']);
        });
    }

    public function down(): void
    {
        Schema::dropIfExists('client_builds');
    }
};
