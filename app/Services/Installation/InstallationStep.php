<?php

namespace App\Services\Installation;

interface InstallationStep
{
    public function title(): string;

    public function shouldRun(InstallationSettings $settings): bool;

    public function run(InstallationSettings $settings): void;
}
