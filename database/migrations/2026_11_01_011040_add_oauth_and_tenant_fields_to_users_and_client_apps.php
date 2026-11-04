<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::table('users', function (Blueprint $table) {
            $table->foreignId('customer_id')->nullable()->after('id')->constrained('customers')->nullOnDelete();
            $table->string('password')->nullable()->change();
            $table->string('google_id')->nullable()->index()->after('email');
            $table->string('apple_id')->nullable()->index()->after('google_id');
            $table->string('avatar_url')->nullable()->after('apple_id');
            $table->string('role', 32)->default('tenant')->after('avatar_url');
        });

        Schema::table('client_apps', function (Blueprint $table) {
            $table->string('primary_color', 16)->default('#2563eb')->after('unattended');
            $table->string('website')->nullable()->after('primary_color');
        });
    }

    public function down(): void
    {
        Schema::table('client_apps', function (Blueprint $table) {
            $table->dropColumn(['primary_color', 'website']);
        });

        Schema::table('users', function (Blueprint $table) {
            $table->dropConstrainedForeignId('customer_id');
            $table->dropColumn(['google_id', 'apple_id', 'avatar_url', 'role']);
        });
    }
};
