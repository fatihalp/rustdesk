<?php

namespace App\Services;

use App\Models\Customer;
use App\Models\User;
use Illuminate\Support\Facades\DB;

class TenantProvisioner
{
    public function register(array $attributes, ?string $company = null): User
    {
        return DB::transaction(function () use ($attributes, $company) {
            $user = User::create($attributes + ['role' => User::ROLE_TENANT]);
            $this->ensureCustomer($user, $company);

            return $user;
        });
    }

    public function ensureCustomer(User $user, ?string $company = null): Customer
    {
        if ($user->customer) {
            return $user->customer;
        }

        return DB::transaction(function () use ($user, $company) {
            $customer = Customer::create([
                'name' => $user->name,
                'company' => $company ?: $user->name,
                'email' => $user->email,
            ]);
            $user->customer()->associate($customer)->save();
            $customer->defaultClientApp()->defaultDownloadLink();

            return $customer;
        });
    }
}
