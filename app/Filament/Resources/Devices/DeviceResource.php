<?php

namespace App\Filament\Resources\Devices;

use App\Filament\Resources\Devices\Pages\ListDevices;
use App\Filament\Resources\Devices\Pages\ViewDevice;
use App\Filament\Resources\Devices\Tables\DevicesTable;
use App\Models\Device;
use BackedEnum;
use Filament\Resources\Resource;
use Filament\Support\Icons\Heroicon;
use Filament\Tables\Table;

class DeviceResource extends Resource
{
    protected static ?string $model = Device::class;

    protected static string|BackedEnum|null $navigationIcon = Heroicon::OutlinedSignal;

    protected static ?string $recordTitleAttribute = 'hostname';

    protected static ?string $modelLabel = 'Cihaz';

    protected static ?string $pluralModelLabel = 'Cihazlar';

    protected static ?int $navigationSort = 0;

    public static function canCreate(): bool
    {
        return false;
    }

    public static function table(Table $table): Table
    {
        return DevicesTable::configure($table);
    }

    public static function getPages(): array
    {
        return [
            'index' => ListDevices::route('/'),
        ];
    }
}
