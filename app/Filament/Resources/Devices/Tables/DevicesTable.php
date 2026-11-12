<?php

namespace App\Filament\Resources\Devices\Tables;

use App\Models\Device;
use App\Services\RelayBridgeService;
use Filament\Actions\Action;
use Filament\Actions\BulkActionGroup;
use Filament\Actions\DeleteBulkAction;
use Filament\Forms\Components\Textarea;
use Filament\Forms\Components\TextInput;
use Filament\Notifications\Notification;
use Filament\Tables\Columns\IconColumn;
use Filament\Tables\Columns\TextColumn;
use Filament\Tables\Filters\Filter;
use Filament\Tables\Table;

class DevicesTable
{
    public static function configure(Table $table): Table
    {
        return $table
            ->modifyQueryUsing(fn ($query) => $query->with(['clientApp.customer']))
            ->defaultSort('last_seen_at', 'desc')
            ->poll('5s')
            ->columns([
                IconColumn::make('online')
                    ->label('')
                    ->state(fn (Device $r) => app(RelayBridgeService::class)->presence($r)['online'])
                    ->boolean()
                    ->trueIcon('heroicon-s-check-circle')
                    ->falseIcon('heroicon-o-minus-circle')
                    ->trueColor('success')
                    ->falseColor('gray'),
                IconColumn::make('unattended_access')
                    ->label('Unattended')
                    ->state(fn (Device $r) => $r->hasUnattendedPassword())
                    ->boolean()
                    ->trueIcon('heroicon-s-lock-open')
                    ->falseIcon('heroicon-s-lock-closed')
                    ->trueColor('success')
                    ->falseColor('warning')
                    ->tooltip(fn (Device $r) => $r->hasUnattendedPassword()
                        ? 'Support agents can connect without a password prompt'
                        : 'Unattended access is disabled on this device (password required)'),
                TextColumn::make('hostname')->label('Computer')->searchable()->weight('bold'),
                TextColumn::make('supportdesk_id')->label('ID')->searchable()->copyable(),
                TextColumn::make('clientApp.customer.name')->label('Customer')->searchable(),
                TextColumn::make('clientApp.app_name')->label('Client App'),
                TextColumn::make('platform')->label('OS')->badge(),
                TextColumn::make('username')->label('User')->toggleable(),
                TextColumn::make('ip_address')->label('IP')->toggleable(isToggledHiddenByDefault: true),
                TextColumn::make('client_version')->label('Version')->toggleable(isToggledHiddenByDefault: true),
                TextColumn::make('last_seen_at')->label('Last Seen')->since()->sortable(),
            ])
            ->filters([
                Filter::make('online')->label('Online Only')->query(fn ($q) => $q->online()),
            ])
            ->recordActions([
                Action::make('open')
                    ->label('Connect')
                    ->icon('heroicon-o-play')
                    ->color('success')
                    ->url(fn (Device $r) => $r->connectUri())
                    ->visible(fn (Device $r) => $r->connectUri() !== null),
                Action::make('connect_pwd')
                    ->label('Connect with Password')
                    ->icon('heroicon-o-key')
                    ->color('gray')
                    ->modalHeading(fn (Device $r) => 'Password Connection: '.($r->hostname ?: $r->supportdesk_id))
                    ->modalDescription('Unattended access is disabled on this device. Ask the user for their Device ID and temporary password.')
                    ->fillForm(fn (Device $r) => [
                        'id' => $r->supportdesk_id,
                        'command' => sprintf('%s --connect %s', $r->appBinary(), $r->supportdesk_id),
                        'uri' => sprintf('%s://connection/new/%s', $r->appBinary(), $r->supportdesk_id),
                    ])
                    ->schema([
                        TextInput::make('id')->label('Device ID')->readOnly()->copyable(),
                        TextInput::make('command')->label('Terminal Command')->readOnly()->copyable(),
                        TextInput::make('uri')->label('Connection URI')->readOnly()->copyable(),
                    ])
                    ->modalSubmitAction(false)
                    ->visible(fn (Device $r) => $r->connectUri() === null),
                Action::make('connect')
                    ->label('CLI')
                    ->icon('heroicon-o-command-line')
                    ->modalHeading(fn (Device $r) => $r->hostname ?: $r->supportdesk_id)
                    ->fillForm(fn (Device $r) => [
                        'command' => $r->connectCommand(),
                        'uri' => $r->connectUri(),
                    ])
                    ->schema([
                        TextInput::make('command')->label('Terminal Command')->readOnly()->copyable(),
                        TextInput::make('uri')->label('SupportDesk URI')->readOnly()->copyable(),
                    ])
                    ->modalSubmitAction(false)
                    ->visible(fn (Device $r) => $r->connectCommand() !== null),
                Action::make('edit_device')
                    ->label('Edit')
                    ->icon('heroicon-o-pencil-square')
                    ->fillForm(fn (Device $r) => [
                        'hostname' => $r->hostname,
                        'notes' => $r->notes,
                    ])
                    ->schema([
                        TextInput::make('hostname')->label('Device Name')->maxLength(255),
                        Textarea::make('notes')->label('Notes')->rows(3)->maxLength(1000),
                    ])
                    ->action(function (Device $r, array $data): void {
                        $r->update($data);
                        Notification::make()->title('Device updated')->success()->send();
                    }),
                Action::make('disconnect')
                    ->label('Disconnect')
                    ->icon('heroicon-o-signal-slash')
                    ->color('warning')
                    ->requiresConfirmation()
                    ->action(function (Device $r): void {
                        app(RelayBridgeService::class)->disconnect($r->supportdesk_id);
                        Notification::make()->title('Session disconnected on RelayServer')->success()->send();
                    }),
            ])
            ->toolbarActions([
                BulkActionGroup::make([DeleteBulkAction::make()]),
            ]);
    }
}
