<?php

namespace App\Services\Installation;

class EnvironmentFile
{
    public function __construct(private readonly string $path) {}

    public function path(): string
    {
        return $this->path;
    }

    public function ensureExists(string $template): void
    {
        if (! is_file($this->path)) {
            copy($template, $this->path);
        }
    }

    public function get(string $key): ?string
    {
        if (! preg_match('/^'.preg_quote($key, '/').'=(.*)$/m', $this->contents(), $match)) {
            return null;
        }

        $value = trim($match[1]);
        if (strlen($value) >= 2 && $value[0] === '"' && str_ends_with($value, '"')) {
            return stripcslashes(substr($value, 1, -1));
        }

        return $value;
    }

    public function keys(): array
    {
        preg_match_all('/^([A-Z0-9_]+)=/m', $this->contents(), $matches);

        return $matches[1];
    }

    public function set(array $values): void
    {
        $contents = $this->contents();

        foreach ($values as $key => $value) {
            $line = $key.'='.$this->format((string) $value);
            $pattern = '/^'.preg_quote($key, '/').'=.*$/m';
            $contents = preg_match($pattern, $contents)
                ? preg_replace_callback($pattern, fn () => $line, $contents)
                : rtrim($contents)."\n".$line."\n";
        }

        file_put_contents($this->path, $contents);
    }

    private function contents(): string
    {
        return is_file($this->path) ? file_get_contents($this->path) : '';
    }

    private function format(string $value): string
    {
        if (preg_match('/^[A-Za-z0-9_.:\/@+=-]*$/', $value)) {
            return $value;
        }

        return '"'.addcslashes($value, '"\\$').'"';
    }
}
