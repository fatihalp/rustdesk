<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Support\Facades\DB;

return new class extends Migration
{
    public function up(): void
    {
        DB::table('users')->whereNull('customer_id')->where('role', 'tenant')->update(['role' => 'admin']);
    }

    public function down(): void
    {
        DB::table('users')->where('role', 'admin')->whereNull('customer_id')->update(['role' => 'tenant']);
    }
};
