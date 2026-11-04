<?php

namespace App\Filament\Resources\ClientApps\Schemas;

use Filament\Forms\Components\FileUpload;
use Filament\Forms\Components\Select;
use Filament\Forms\Components\TextInput;
use Filament\Forms\Components\Textarea;
use Filament\Forms\Components\Toggle;
use Filament\Schemas\Schema;

class ClientAppForm
{
    public static function configure(Schema $schema): Schema
    {
        return $schema
            ->components([
                Select::make('customer_id')
                    ->label('Customer')
                    ->relationship('customer', 'name')
                    ->searchable()
                    ->preload()
                    ->required()
                    ->createOptionForm([
                        TextInput::make('name')->label('Name')->required(),
                        TextInput::make('email')->email(),
                    ]),
                TextInput::make('app_name')
                    ->label('Application Name')
                    ->required()
                    ->maxLength(60),
                FileUpload::make('logo_path')
                    ->label('Logo')
                    ->image()
                    ->imageEditor()
                    ->disk('public')
                    ->directory('client-logos')
                    ->visibility('public')
                    ->maxSize(4096)
                    ->helperText('PNG, at least 512x512 pixels recommended.'),
                Toggle::make('unattended')
                    ->label('Unattended Access')
                    ->helperText('When enabled, support agents can connect without a customer prompt or password.')
                    ->default(true),
                Toggle::make('is_active')
                    ->label('Active')
                    ->default(true),
                Textarea::make('notes')
                    ->label('Notes')
                    ->columnSpanFull(),
            ]);
    }
}
