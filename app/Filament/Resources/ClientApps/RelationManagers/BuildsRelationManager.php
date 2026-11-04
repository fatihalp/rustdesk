<?php

namespace App\Filament\Resources\ClientApps\RelationManagers;

use App\Filament\Actions\StartBuildAction;
use App\Models\ClientBuild;
use Filament\Actions\Action;
use Filament\Resources\RelationManagers\RelationManager;
use Filament\Tables\Columns\IconColumn;
use Filament\Tables\Columns\TextColumn;
use Filament\Tables\Table;

class BuildsRelationManager extends RelationManager
{
    protected static string $relationship = 'builds';

    protected static ?string $title = 'Builds';

    public function table(Table $table): Table
    {
        return $table
            ->defaultSort('id', 'desc')
            ->poll('10s')
            ->headerActions([
                Action::make('build_macos')
                    ->label('Build macOS')
                    ->icon('heroicon-o-arrow-down-tray')
                    ->button()
                    ->color('primary')
                    ->requiresConfirmation()
                    ->action(fn () => $this->triggerBuild('macos')),
                Action::make('build_windows')
                    ->label('Build Windows')
                    ->icon('heroicon-o-arrow-down-tray')
                    ->button()
                    ->color('gray')
                    ->requiresConfirmation()
                    ->action(fn () => $this->triggerBuild('windows')),
            ])
            ->emptyStateHeading('No builds yet')
            ->emptyStateDescription('Build a macOS or Windows package for this client application to make it ready for download.')
            ->emptyStateActions([
                Action::make('empty_build_macos')
                    ->label('Build macOS')
                    ->icon('heroicon-o-arrow-down-tray')
                    ->button()
                    ->color('primary')
                    ->requiresConfirmation()
                    ->action(fn () => $this->triggerBuild('macos')),
                Action::make('empty_build_windows')
                    ->label('Build Windows')
                    ->icon('heroicon-o-arrow-down-tray')
                    ->button()
                    ->color('gray')
                    ->requiresConfirmation()
                    ->action(fn () => $this->triggerBuild('windows')),
            ])
            ->columns([
                TextColumn::make('platform')->label('Platform')->badge(),
                TextColumn::make('status')->label('Status')->badge()->color(fn (string $state) => match ($state) {
                    ClientBuild::READY => 'success',
                    ClientBuild::FAILED => 'danger',
                    ClientBuild::BUILDING => 'warning',
                    default => 'gray',
                }),
                IconColumn::make('signed')->label('Signed')->boolean(),
                TextColumn::make('version')->label('Version'),
                TextColumn::make('size_bytes')->label('Size')->formatStateUsing(
                    fn ($state) => $state ? number_format($state / 1048576, 1).' MB' : null
                ),
                TextColumn::make('error')->label('Error')->limit(60)->tooltip(fn ($state) => $state),
                TextColumn::make('github_run_url')->label('Run')->url(fn ($record) => $record->github_run_url, true)->limit(20),
                TextColumn::make('created_at')->label('Created')->since(),
            ])
            ->recordActions([
                Action::make('download')
                    ->label('Download')
                    ->icon('heroicon-o-arrow-down-tray')
                    ->color('success')
                    ->url(fn (ClientBuild $r) => route('admin.build.download', $r), shouldOpenInNewTab: true)
                    ->visible(fn (ClientBuild $r) => $r->status === ClientBuild::READY && ! empty($r->artifact_path)),
            ]);
    }

    protected function triggerBuild(string $platform): void
    {
        StartBuildAction::run($this->getOwnerRecord(), $platform);
    }
}
