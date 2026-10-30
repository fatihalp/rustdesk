<?php

namespace Database\Seeders;

use App\Models\User;
use Illuminate\Database\Seeder;

class DatabaseSeeder extends Seeder
{
    public function run(): void
    {
        $password = config('panel.admin.password');
        if (! $password) {
            $this->command?->warn('PANEL_ADMIN_PASSWORD is not set; skipping admin user.');

            return;
        }

        User::updateOrCreate(
            ['email' => config('panel.admin.email')],
            ['name' => 'Admin', 'password' => $password],
        );
    }
}
