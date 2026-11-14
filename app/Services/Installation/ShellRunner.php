<?php

namespace App\Services\Installation;

use Illuminate\Support\Facades\Process;
use RuntimeException;
use Symfony\Component\Console\Output\OutputInterface;

class ShellRunner
{
    private ?OutputInterface $output = null;

    public function streamTo(OutputInterface $output): void
    {
        $this->output = $output;
    }

    public function run(array|string $command, array $environment = [], ?string $path = null): string
    {
        $result = Process::forever()
            ->env($environment)
            ->path($path ?? base_path())
            ->run($command, fn (string $type, string $buffer) => $this->output?->write($buffer));

        if ($result->failed()) {
            throw new RuntimeException("Command failed with exit code {$result->exitCode()}: {$result->command()}");
        }

        return trim($result->output());
    }

    public function succeeds(array|string $command): bool
    {
        return Process::path('/')->run($command)->successful();
    }
}
