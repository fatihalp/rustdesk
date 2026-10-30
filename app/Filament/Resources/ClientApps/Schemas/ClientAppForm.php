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
                    ->label('Müşteri')
                    ->relationship('customer', 'name')
                    ->searchable()
                    ->preload()
                    ->required()
                    ->createOptionForm([
                        TextInput::make('name')->label('Ad')->required(),
                        TextInput::make('email')->email(),
                    ]),
                TextInput::make('app_name')
                    ->label('Uygulama adı')
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
                    ->helperText('PNG, en az 512×512 piksel önerilir.'),
                Toggle::make('unattended')
                    ->label('Şifresiz (gözetimsiz) bağlantı')
                    ->helperText('Açıkken müşteri onay vermeden, şifre sorulmadan bağlanabilirsiniz.')
                    ->default(true),
                Toggle::make('is_active')
                    ->label('Aktif')
                    ->default(true),
                Textarea::make('notes')
                    ->label('Not')
                    ->columnSpanFull(),
            ]);
    }
}
