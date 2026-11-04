<?php

namespace Database\Seeders;

use App\Models\ClientApp;
use App\Models\Customer;
use Illuminate\Database\Seeder;

class CustomerSeeder extends Seeder
{
    public function run(): void
    {
        $customers = [
            [
                'name' => 'Visiosoft',
                'company' => 'Visiosoft Technology Inc.',
                'email' => 'info@visiosoft.com.tr',
                'phone' => '+90 216 000 00 00',
                'notes' => 'Visiosoft enterprise support client and live device connection.',
                'apps' => [
                    [
                        'app_name' => 'Visiosoft',
                        'unattended' => true,
                        'is_active' => true,
                        'notes' => 'Visiosoft custom branded remote support client',
                    ],
                ],
            ],
            [
                'name' => 'Alex Turner',
                'company' => 'Turner IT Solutions Inc.',
                'email' => 'alex@turnerit.com',
                'phone' => '+1 555 111 2233',
                'notes' => 'Priority enterprise support customer with 25 active device licenses.',
                'apps' => [
                    [
                        'app_name' => 'Turner IT Support',
                        'unattended' => true,
                        'is_active' => true,
                        'notes' => 'Headquarters server and staff workstations',
                    ],
                ],
            ],
            [
                'name' => 'Sarah Miller',
                'company' => 'Miller Global Logistics Ltd.',
                'email' => 'sarah@millerlogistics.com',
                'phone' => '+1 555 222 3344',
                'notes' => '24/7 remote access support for warehouse and shipping terminals.',
                'apps' => [
                    [
                        'app_name' => 'Miller Logistics Quick Help',
                        'unattended' => false,
                        'is_active' => true,
                        'notes' => 'User-confirmed on-demand support client',
                    ],
                ],
            ],
            [
                'name' => 'Michael Carter',
                'company' => 'Carter Financial Advisory & Audit',
                'email' => 'michael@carterfinance.com',
                'phone' => '+1 555 333 4455',
                'notes' => 'Annual maintenance and data security SLA customer.',
                'apps' => [
                    [
                        'app_name' => 'Carter Finance Remote Access',
                        'unattended' => true,
                        'is_active' => true,
                        'notes' => 'Financial advisor and analyst workstations',
                    ],
                ],
            ],
        ];

        foreach ($customers as $item) {
            $apps = $item['apps'] ?? [];
            unset($item['apps']);

            $customer = Customer::updateOrCreate(
                ['email' => $item['email']],
                $item
            );

            foreach ($apps as $appData) {
                ClientApp::firstOrCreate(
                    [
                        'customer_id' => $customer->id,
                        'app_name' => $appData['app_name'],
                    ],
                    $appData
                );
            }
        }
    }
}
