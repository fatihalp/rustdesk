<?php

namespace App\Filament\Widgets;

use App\Models\ClientApp;
use App\Models\Device;
use App\Services\RelayBridgeService;
use Filament\Widgets\StatsOverviewWidget;
use Filament\Widgets\StatsOverviewWidget\Stat;

class DeviceStats extends StatsOverviewWidget
{
    protected ?string $pollingInterval = '5s';

    protected function getStats(): array
    {
        $bridge = app(RelayBridgeService::class);
        $relay = $bridge->status();
        $onlineCount = Device::online()->count();

        return [
            Stat::make('Online Devices', $onlineCount)
                ->description($relay['reachable'] ? 'Relay server connected' : 'Database heartbeat state')
                ->color('success'),
            Stat::make('Active Relay Sessions', count($relay['sessions'])),
            Stat::make('Total Devices', Device::count()),
            Stat::make('Client Apps', ClientApp::count()),
        ];
    }
}
