<?php

namespace App\Filament\Resources\ClientApps\Tables;

use App\Models\ClientApp;
use App\Models\ClientBuild;
use App\Models\DownloadLink;
use App\Services\GitHubWorkflowDispatcher;
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
                TextColumn::make('app_name')->label('Uygulama')->searchable()->weight('bold'),
                TextColumn::make('customer.name')->label('Müşteri')->searchable(),
                TextColumn::make('devices_count')->label('Cihaz')->counts('devices')->badge(),
                IconColumn::make('unattended')->label('Şifresiz')->boolean(),
                IconColumn::make('is_active')->label('Aktif')->boolean(),
            ])
            ->recordActions([
                ActionGroup::make([
                    self::buildAction('windows', 'Windows derle'),
                    self::buildAction('macos', 'macOS derle'),
                ])->label('Derle')->icon('heroicon-o-cog-6-tooth')->button(),
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
            ->action(function (ClientApp $record) use ($platform) {
                $build = $record->builds()->create(['platform' => $platform, 'status' => ClientBuild::PENDING]);
                try {
                    app(GitHubWorkflowDispatcher::class)->queue($build);
                } catch (\Throwable $e) {
                    Notification::make()->title('Derleme başlatılamadı')->body($e->getMessage())->danger()->send();

                    return;
                }
                Notification::make()->title('Derleme kuyruğa alındı')->success()->send();
            });
    }

    private static function linkAction(): Action
    {
        return Action::make('create_link')
            ->label('İndirme linki')
            ->icon('heroicon-o-link')
            ->schema([
                TextInput::make('label')->label('Etiket'),
                TextInput::make('days')->label('Geçerlilik (gün)')->numeric()->default(7)->minValue(1),
                TextInput::make('max_downloads')->label('En fazla indirme')->numeric()->minValue(1),
            ])
            ->action(function (ClientApp $record, array $data) {
                $link = $record->downloadLinks()->create([
                    'label' => $data['label'] ?? null,
                    'expires_at' => ! empty($data['days']) ? now()->addDays((int) $data['days']) : null,
                    'max_downloads' => $data['max_downloads'] ?: null,
                ]);
                self::notifyLink($link);
            });
    }

    public static function notifyLink(DownloadLink $link): void
    {
        Notification::make()
            ->title('Link oluşturuldu')
            ->body($link->url())
            ->success()
            ->persistent()
            ->send();
    }
}
