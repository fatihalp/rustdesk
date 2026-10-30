<?php

namespace App\Filament\Resources\Devices\Tables;

use App\Models\Device;
use Filament\Actions\Action;
use Filament\Actions\BulkActionGroup;
use Filament\Actions\DeleteBulkAction;
use Filament\Forms\Components\TextInput;
use Filament\Tables\Columns\IconColumn;
use Filament\Tables\Columns\TextColumn;
use Filament\Tables\Filters\Filter;
use Filament\Tables\Table;

class DevicesTable
{
    public static function configure(Table $table): Table
    {
        return $table
            ->defaultSort('last_seen_at', 'desc')
            ->poll('10s')
            ->columns([
                IconColumn::make('online')
                    ->label('')
                    ->state(fn (Device $r) => $r->isOnline())
                    ->boolean()
                    ->trueIcon('heroicon-s-check-circle')
                    ->falseIcon('heroicon-o-minus-circle')
                    ->trueColor('success')
                    ->falseColor('gray'),
                TextColumn::make('hostname')->label('Bilgisayar')->searchable()->weight('bold'),
                TextColumn::make('supportdesk_id')->label('ID')->searchable()->copyable(),
                TextColumn::make('clientApp.customer.name')->label('Müşteri')->searchable(),
                TextColumn::make('clientApp.app_name')->label('İstemci'),
                TextColumn::make('platform')->label('OS')->badge(),
                TextColumn::make('username')->label('Kullanıcı')->toggleable(),
                TextColumn::make('ip_address')->label('IP')->toggleable(isToggledHiddenByDefault: true),
                TextColumn::make('client_version')->label('Sürüm')->toggleable(isToggledHiddenByDefault: true),
                TextColumn::make('last_seen_at')->label('Son görülme')->since()->sortable(),
            ])
            ->filters([
                Filter::make('online')->label('Çevrimiçi')->query(fn ($q) => $q->online()),
            ])
            ->recordActions([
                Action::make('open')
                    ->label('Bağlan')
                    ->icon('heroicon-o-play')
                    ->url(fn (Device $r) => $r->connectUri())
                    ->visible(fn (Device $r) => $r->connectUri() !== null),
                Action::make('connect')
                    ->label('Komut')
                    ->icon('heroicon-o-command-line')
                    ->modalHeading(fn (Device $r) => $r->hostname ?: $r->supportdesk_id)
                    ->fillForm(fn (Device $r) => [
                        'command' => $r->connectCommand(),
                        'uri' => $r->connectUri(),
                    ])
                    ->schema([
                        TextInput::make('command')->label('Terminal komutu')->readOnly()->copyable(),
                        TextInput::make('uri')->label('SupportDesk bağlantısı')->readOnly()->copyable(),
                    ])
                    ->modalSubmitAction(false)
                    ->visible(fn (Device $r) => $r->connectCommand() !== null),
            ])
            ->toolbarActions([
                BulkActionGroup::make([DeleteBulkAction::make()]),
            ]);
    }
}
