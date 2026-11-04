<?php

namespace Database\Seeders;

use App\Models\User;
use Illuminate\Database\Seeder;

class DefaultAdminSeeder extends Seeder
{
    public function run(): void
    {
        $email = env('PANEL_ADMIN_EMAIL', 'a@a.com');
        $password = env('PANEL_ADMIN_PASSWORD');

        if (! $password) {
            $this->command?->warn('PANEL_ADMIN_PASSWORD is not set; skipping default admin user.');

            return;
        }

        User::updateOrCreate(
            ['email' => $email],
            ['name' => 'Admin', 'password' => $password],
        );
    }
}
