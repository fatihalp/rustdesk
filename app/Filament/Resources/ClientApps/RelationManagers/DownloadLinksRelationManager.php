<?php

namespace App\Filament\Resources\ClientApps\RelationManagers;

use App\Filament\Resources\ClientApps\Tables\ClientAppsTable;
use App\Models\DownloadLink;
use Filament\Actions\Action;
use Filament\Actions\CreateAction;
use Filament\Forms\Components\DateTimePicker;
use Filament\Forms\Components\TextInput;
use Filament\Resources\RelationManagers\RelationManager;
use Filament\Schemas\Schema;
use Filament\Tables\Columns\TextColumn;
use Filament\Tables\Table;

class DownloadLinksRelationManager extends RelationManager
{
    protected static string $relationship = 'downloadLinks';

    protected static ?string $title = 'Download Links';

    public function form(Schema $schema): Schema
    {
        return $schema->components([
            TextInput::make('label')->label('Label')->default('Custom Download Link'),
            DateTimePicker::make('expires_at')->label('Expires At'),
            TextInput::make('max_downloads')->label('Max Downloads')->numeric()->minValue(1),
        ]);
    }

    public function table(Table $table): Table
    {
        return $table
            ->defaultSort('id', 'desc')
            ->columns([
                TextColumn::make('label')->label('Label')->placeholder('-'),
                TextColumn::make('url')
                    ->label('Link')
                    ->state(fn (DownloadLink $record) => $record->url())
                    ->url(fn (DownloadLink $record) => $record->url(), shouldOpenInNewTab: true)
                    ->color('primary')
                    ->copyable()
                    ->copyMessage('Download link copied')
                    ->tooltip('Click to open in a new tab or copy'),
                TextColumn::make('download_count')->label('Downloads')
                    ->formatStateUsing(fn ($state, DownloadLink $r) => $state.($r->max_downloads ? " / {$r->max_downloads}" : '')),
                TextColumn::make('expires_at')->label('Expires At')->dateTime()->placeholder('Unlimited'),
                TextColumn::make('status')->label('Status')->badge()
                    ->state(fn (DownloadLink $r) => $r->revoked_at ? 'Revoked' : ($r->isUsable() ? 'Valid' : 'Expired'))
                    ->color(fn (string $state) => $state === 'Valid' ? 'success' : 'gray'),
            ])
            ->headerActions([
                CreateAction::make()
                    ->label('Create Download Link')
                    ->icon('heroicon-o-plus')
                    ->after(fn (DownloadLink $record) => ClientAppsTable::notifyLink($record)),
                Action::make('open_shortlink')
                    ->label(fn () => 'Permanent Short Link: '.url('/s/'.$this->getOwnerRecord()->slug))
                    ->icon('heroicon-o-link')
                    ->url(fn () => url('/s/'.$this->getOwnerRecord()->slug), shouldOpenInNewTab: true)
                    ->color('gray'),
            ])
            ->recordActions([
                Action::make('open')
                    ->label('Open')
                    ->icon('heroicon-o-arrow-top-right-on-square')
                    ->url(fn (DownloadLink $r) => $r->url(), shouldOpenInNewTab: true),
                Action::make('revoke')
                    ->label('Revoke')
                    ->color('danger')
                    ->requiresConfirmation()
                    ->visible(fn (DownloadLink $r) => $r->revoked_at === null)
                    ->action(fn (DownloadLink $r) => $r->update(['revoked_at' => now()])),
            ]);
    }
}
