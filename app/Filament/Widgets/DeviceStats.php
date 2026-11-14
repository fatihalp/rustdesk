<?php

namespace App\Filament\Widgets;

use App\Models\Customer;
use App\Models\Device;
use App\Services\RelayBridgeService;
use Filament\Widgets\StatsOverviewWidget;
use Filament\Widgets\StatsOverviewWidget\Stat;

class DeviceStats extends StatsOverviewWidget
{
    protected static ?int $sort = 1;

    protected ?string $pollingInterval = '10s';

    protected function getColumns(): int
    {
        return 3;
    }

    protected function getStats(): array
    {
        $relay = app(RelayBridgeService::class)->status();
        $totalDevices = Device::count();
        $activeThisWeek = Device::activeWithinDays(7)->count();
        $activeThisFortnight = Device::activeWithinDays(15)->count();

        return [
            Stat::make('Active in Last 7 Days', $activeThisWeek)
                ->description($this->shareOf($activeThisWeek, $totalDevices))
                ->color('primary'),
            Stat::make('Active in Last 15 Days', $activeThisFortnight)
                ->description($this->shareOf($activeThisFortnight, $totalDevices))
                ->color('primary'),
            Stat::make('Online Now', Device::online()->count())
                ->description($relay['reachable'] ? 'Relay server connected' : 'Relay server unreachable')
                ->color($relay['reachable'] ? 'success' : 'danger'),
            Stat::make('Total Devices', $totalDevices)
                ->description(Device::where('first_seen_at', '>=', now()->subDays(7))->count().' new this week'),
            Stat::make('Customers', Customer::count()),
            Stat::make('Sessions in Progress', count($relay['sessions'])),
        ];
    }

    private function shareOf(int $activeDevices, int $totalDevices): string
    {
        if ($totalDevices === 0) {
            return 'No devices enrolled yet';
        }

        return round($activeDevices / $totalDevices * 100).'% of all devices';
    }
}
