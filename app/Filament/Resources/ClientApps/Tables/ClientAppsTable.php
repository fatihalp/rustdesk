<?php

namespace App\Filament\Resources\ClientApps\Tables;

use App\Models\ClientApp;
use App\Models\DownloadLink;
use App\Filament\Actions\StartBuildAction;
use Filament\Actions\Action;
use Filament\Actions\ActionGroup;
use Filament\Actions\BulkActionGroup;
use Filament\Actions\DeleteBulkAction;
use Filament\Actions\EditAction;
use Filament\Forms\Components\TextInput;
use Filament\Notifications\Notification;
use Filament\Tables\Columns\ImageColumn;
use Filament\Tables\Columns\IconColumn;
use Filament\Tables\Columns\TextColumn;
use Filament\Tables\Table;

class ClientAppsTable
{
    public static function configure(Table $table): Table
    {
        return $table
            ->columns([
                ImageColumn::make('logo_path')->label('')->disk('public')->circular(),
                TextColumn::make('app_name')->label('Application')->searchable()->weight('bold'),
                TextColumn::make('customer.name')->label('Customer')->searchable(),
                TextColumn::make('devices_count')->label('Devices')->counts('devices')->badge(),
                IconColumn::make('unattended')->label('Unattended')->boolean(),
                IconColumn::make('is_active')->label('Active')->boolean(),
            ])
            ->recordActions([
                ActionGroup::make([
                    self::buildAction('windows', 'Build Windows'),
                    self::buildAction('macos', 'Build macOS'),
                ])->label('Build')->icon('heroicon-o-cog-6-tooth')->button(),
                self::linkAction(),
                EditAction::make(),
            ])
            ->toolbarActions([
                BulkActionGroup::make([DeleteBulkAction::make()]),
            ]);
    }

    private static function buildAction(string $platform, string $label): Action
    {
        return Action::make("build_$platform")
            ->label($label)
            ->requiresConfirmation()
            ->action(fn (ClientApp $record) => StartBuildAction::run($record, $platform));
    }

    private static function linkAction(): Action
    {
        return Action::make('create_link')
            ->label('Download Link')
            ->icon('heroicon-o-link')
            ->schema([
                TextInput::make('label')->label('Label')->default('Custom Download Link'),
                TextInput::make('days')->label('Validity (days)')->numeric()->default(7)->minValue(1),
                TextInput::make('max_downloads')->label('Max Downloads')->numeric()->minValue(1),
            ])
            ->action(function (ClientApp $record, array $data) {
                $link = $record->downloadLinks()->create([
                    'label' => $data['label'] ?? 'Custom Download Link',
                    'expires_at' => ! empty($data['days']) ? now()->addDays((int) $data['days']) : null,
                    'max_downloads' => $data['max_downloads'] ?: null,
                ]);
                self::notifyLink($link);
            });
    }

    public static function notifyLink(DownloadLink $link): void
    {
        Notification::make()
            ->title('Download Link Created')
            ->body($link->url())
            ->actions([
                \Filament\Notifications\Actions\Action::make('open')
                    ->label('Open Link')
                    ->url($link->url(), shouldOpenInNewTab: true)
                    ->button(),
            ])
            ->success()
            ->persistent()
            ->send();
    }
}
