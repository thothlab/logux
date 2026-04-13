# logux

[![en](https://img.shields.io/badge/lang-English-blue)](README_EN.md)

**Android Logs & Traffic CLI** -- интерактивная утилита для Android-разработчиков: просмотр логов в реальном времени, фильтрация, инспекция трафика и подмена сетевых ответов.

---

## Возможности

- **Логи через ADB** -- чтение `adb logcat` с цветным форматированным выводом
- **Smart-фильтрация** -- по package, tag, level, PID, regex, тексту -- все меняется на лету без перезапуска
- **Отслеживание приложения** -- автоматический PID-трекинг с обновлением при рестарте
- **5 пресетов вывода** -- compact, threadtime, verbose, minimal, json
- **Инспекция трафика** -- HTTP/HTTPS proxy через mitmproxy/mitmdump
- **Mock-правила** -- подмена ответов через YAML-конфигурацию с hot reload
- **Интерактивный CLI** -- REPL с автодополнением, историей команд и подсказками

## Требования

- [Rust](https://rustup.rs/) 1.70+
- [ADB](https://developer.android.com/tools/adb) (Android Debug Bridge)
- [mitmproxy](https://mitmproxy.org/) (опционально, для инспекции трафика)

## Установка

### 1. Установить Rust (если ещё не установлен)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

### 2. Собрать из исходников

```bash
git clone https://github.com/thothlab/logux.git
cd logux
cargo build --release
```

Бинарник: `./target/release/logux` (3.5 МБ, без внешних зависимостей)

### Установка в систему

```bash
cargo install --path .
```

После этого `logux` доступен из любой директории.

## Быстрый старт

```bash
# Запуск
logux

# Внутри REPL:
/devices              # Список подключённых устройств
/app com.example.app  # Фильтр по приложению (авто PID-трекинг)
/level W              # Показывать только WARN и выше
/grep error           # Поиск по тексту с подсветкой
/format json          # Переключить формат на JSON
```

## Команды

### Общие

| Команда | Описание |
|---------|----------|
| `/help` | Справка по командам |
| `/exit` | Выход |
| `/clear` | Очистить экран |

### ADB

| Команда | Описание |
|---------|----------|
| `/devices` | Список устройств |
| `/connect <ip:port>` | Подключиться по TCP |
| `/disconnect` | Отключиться |

### Логи

| Команда | Описание |
|---------|----------|
| `/app <package>` | Фильтр по приложению (smart PID tracking) |
| `/pid <pid>` | Фильтр по PID |
| `/tag <tag>` | Фильтр по тегу |
| `/level <V\|D\|I\|W\|E\|F>` | Минимальный уровень логов |
| `/grep <text>` | Поиск по тексту (без учёта регистра) |
| `/regex <pattern>` | Поиск по регулярному выражению |
| `/filter reset` | Сбросить все фильтры |
| `/filter show` | Показать активные фильтры |

### Формат

| Команда | Описание |
|---------|----------|
| `/format <preset>` | compact / threadtime / verbose / minimal / json |
| `/fields +field -field` | Включить/выключить поля: timestamp, level, tag, pid, tid |

### Управление

| Команда | Описание |
|---------|----------|
| `/pause` | Приостановить вывод |
| `/resume` | Возобновить вывод |
| `/save <file>` | Сохранять подходящие логи в файл |

### Пресеты

| Команда | Описание |
|---------|----------|
| `/preset save <name>` | Сохранить текущую конфигурацию |
| `/preset load <name>` | Загрузить пресет |
| `/preset list` | Список пресетов |
| `/preset delete <name>` | Удалить пресет |

### Трафик

| Команда | Описание |
|---------|----------|
| `/traffic open` | Запустить proxy |
| `/traffic close` | Остановить proxy |
| `/traffic list` | Показать перехваченные запросы |
| `/traffic inspect <id>` | Детали запроса/ответа |
| `/traffic filter <expr>` | Фильтр: host=, path=, method=, status= |
| `/traffic clear` | Очистить перехваченный трафик |

### Mock-правила

| Команда | Описание |
|---------|----------|
| `/mock load <file.yaml>` | Загрузить правила |
| `/mock list` | Список правил |
| `/mock enable <id>` | Включить правило |
| `/mock disable <id>` | Выключить правило |
| `/mock reload` | Перезагрузить правила из файла |

## Пример YAML mock-правил

```yaml
rules:
  - id: user_profile_mock
    enabled: true
    priority: 10
    match:
      method: GET
      path: /api/v1/profile
      query:
        userId: "123"
    response:
      type: file
      file: mocks/profile_123.json
      status: 200

  - id: force_error
    enabled: false
    match:
      path: /api/v1/payment
    response:
      type: error
      status: 500
```

## Архитектура

```
src/
 ├── main.rs              -- точка входа (tokio async runtime)
 ├── adb/mod.rs           -- управление устройствами, logcat streaming
 ├── cli/
 │   ├── shell.rs          -- интерактивный REPL
 │   ├── commands.rs       -- обработчики команд
 │   └── completer.rs      -- автодополнение
 ├── logs/
 │   ├── parser.rs         -- парсер logcat (threadtime/brief)
 │   ├── filters.rs        -- composable-фильтры
 │   └── formatter.rs      -- цветной вывод, пресеты
 ├── traffic/mod.rs        -- proxy-адаптер
 ├── mock/mod.rs           -- YAML rules engine
 └── config/mod.rs         -- система пресетов
```

## Версии

| Тег | Язык | Описание |
|-----|------|----------|
| `v2.0.0` | Rust | Текущая версия -- единый бинарник 3.5 МБ |
| `v1.0.0-python` | Python | Предыдущая версия (prompt_toolkit + rich + mitmproxy) |

```bash
# Клонировать Python-версию
git clone --branch v1.0.0-python https://github.com/thothlab/logux.git
```

## Лицензия

MIT
