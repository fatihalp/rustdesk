<?php

namespace Database\Seeders;

use App\Models\User;
use Illuminate\Database\Seeder;

class DefaultAdminSeeder extends Seeder
{
    public function run(): void
    {
        $email = config('panel.admin.email');
        $password = config('panel.admin.password');

        if (! $password) {
            $this->command?->warn('PANEL_ADMIN_PASSWORD is not set; skipping default admin user.');

            return;
        }

        User::updateOrCreate(
            ['email' => $email],
            ['name' => 'Admin', 'password' => $password, 'role' => User::ROLE_ADMIN],
        );
    }
}
