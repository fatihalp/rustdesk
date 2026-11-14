<?php

namespace App\Filament\Widgets;

use App\Filament\Resources\ClientApps\ClientAppResource;
use App\Models\ClientApp;
use Filament\Tables\Columns\TextColumn;
use Filament\Tables\Table;
use Filament\Widgets\TableWidget;

class CustomerUsage extends TableWidget
{
    protected static ?int $sort = 2;

    protected int|string|array $columnSpan = 'full';

    public function table(Table $table): Table
    {
        return $table
            ->heading('Usage by Customer')
            ->query(
                ClientApp::query()
                    ->with('customer')
                    ->withCount([
                        'devices',
                        'devices as active_week_count' => fn ($query) => $query->activeWithinDays(7),
                        'devices as active_fortnight_count' => fn ($query) => $query->activeWithinDays(15),
                    ])
                    ->withMax('devices', 'last_seen_at')
            )
            ->defaultSort('active_week_count', 'desc')
            ->paginated([10, 25])
            ->recordUrl(fn (ClientApp $record) => ClientAppResource::getUrl('edit', ['record' => $record]))
            ->emptyStateHeading('No client apps yet')
            ->columns([
                TextColumn::make('customer.name')->label('Customer')->weight('bold')->searchable(),
                TextColumn::make('app_name')->label('Application')->searchable(),
                TextColumn::make('active_week_count')->label('Active 7 Days')->numeric()->sortable(),
                TextColumn::make('active_fortnight_count')->label('Active 15 Days')->numeric()->sortable(),
                TextColumn::make('devices_count')->label('Total Devices')->numeric()->sortable(),
                TextColumn::make('devices_max_last_seen_at')->label('Last Activity')->since()->placeholder('Never')->sortable(),
            ]);
    }
}
