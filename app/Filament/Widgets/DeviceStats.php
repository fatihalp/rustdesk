<?php

namespace App\Filament\Widgets;

use App\Models\ClientApp;
use App\Models\Device;
use Filament\Widgets\StatsOverviewWidget;
use Filament\Widgets\StatsOverviewWidget\Stat;

class DeviceStats extends StatsOverviewWidget
{
    protected ?string $pollingInterval = '10s';

    protected function getStats(): array
    {
        return [
            Stat::make('Çevrimiçi cihaz', Device::online()->count())->color('success'),
            Stat::make('Toplam cihaz', Device::count()),
            Stat::make('İstemci', ClientApp::count()),
        ];
    }
}
