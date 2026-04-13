# logux

[![en](https://img.shields.io/badge/lang-English-blue)](README_EN.md)

**Android Logs & Traffic CLI** -- TUI-утилита для Android-разработчиков: просмотр логов в реальном времени с колоночной раскладкой, фильтрация, инспекция трафика и подмена сетевых ответов.

---

## Возможности

- **TUI с разделённым экраном** -- логи прокручиваются вверху, строка ввода всегда видна внизу
- **Колоночный вывод логов** -- timestamp, level, tag, message в фиксированных колонках; длинные сообщения переносятся внутри колонки message
- **Логи через ADB** -- чтение `adb logcat` с цветным форматированным выводом
- **Smart-фильтрация** -- по package, tag, level, PID, regex, тексту -- все меняется на лету без перезапуска
- **Фильтры-исключения** -- `/exclude tag` и `/exclude msg` для скрытия ненужных строк (аналог LogRabbit "None of")
- **Редактирование фильтров** -- `/filter edit` загружает текущие фильтры в строку ввода для редактирования
- **Отслеживание приложения** -- автоматический PID-трекинг с обновлением при рестарте
- **5 пресетов вывода** -- compact, threadtime, verbose, minimal, json
- **Авто-подключение** -- единственное устройство выбирается автоматически; при нескольких -- выбор из списка
- **Умное автодополнение** -- `/app` показывает историю пакетов и текущее foreground-приложение; `/filter` показывает пресеты, связанные с текущим приложением
- **Инспекция трафика** -- HTTP/HTTPS proxy через mitmproxy/mitmdump
- **Mock-правила** -- подмена ответов через YAML-конфигурацию с hot reload
- **Горячие клавиши** -- PageUp/Down прокрутка, Ctrl+C выход, Ctrl+L очистка, Tab автодополнение

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

Бинарник: `./target/release/logux`

### Установка в систему

```bash
cargo install --path .
```

После этого `logux` доступен из любой директории.

### Обновление

```bash
cd logux && git pull && cargo build --release && cargo install --path .
```

## Быстрый старт

```bash
# Запуск
logux

# Внутри TUI:
/devices              # Список подключённых устройств
/app com.example.app  # Фильтр по приложению (авто PID-трекинг)
/level W              # Показывать только WARN и выше
/grep error           # Поиск по тексту с подсветкой
/format json          # Переключить формат на JSON
/stop                 # Остановить поток логов
```

## Интерфейс

```
┌──────────────────────────────────────────────────────┐
│ 04-13 12:34:56  D  MyTag          Short message      │ ← логи с колонками
│ 04-13 12:34:57  W  NetworkManager This is a long     │
│                                    message that wraps │ ← перенос внутри колонки
│ ...                                                   │
├──────────────────────────────────────────────────────┤
│  device_name   com.pkg   STREAMING         120 lines │ ← статус-бар
├──────────────────────────────────────────────────────┤
│ logux > /app mts_                                     │ ← строка ввода (всегда видна)
└──────────────────────────────────────────────────────┘
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
| `/tag <tag>` | Добавить тег-фильтр (`-tag` убрать, `reset` сбросить) |
| `/level <V\|D\|I\|W\|E\|F>` | Минимальный уровень (`reset` сбросить) |
| `/grep <text>` | Поиск по тексту (`reset` сбросить) |
| `/regex <pattern>` | Поиск по regex (`reset` сбросить) |
| `/exclude tag <name>` | Исключить тег из вывода |
| `/exclude msg <text>` | Исключить строки с текстом |
| `/exclude show` | Показать исключения |
| `/exclude reset` | Сбросить все исключения |
| `/exclude remove <value>` | Убрать одно исключение |
| `/filter reset` | Сбросить все фильтры |
| `/filter show` | Показать активные фильтры |
| `/filter edit` | Редактировать фильтры в строке ввода |
| `/filter set <expr>` | Задать фильтры одной строкой |
| `/filter <preset>` | Загрузить пресет фильтров |

### Формат

| Команда | Описание |
|---------|----------|
| `/format <preset>` | compact / threadtime / verbose / minimal / json |
| `/fields +field -field` | Включить/выключить поля: timestamp, level, tag, pid, tid |

### Управление

| Команда | Описание |
|---------|----------|
| `/stop` | Полная остановка потока логов |
| `/pause` | Пауза/возобновление (toggle) |
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

### Горячие клавиши

| Клавиша | Действие |
|---------|----------|
| `PageUp` / `PageDown` | Прокрутка логов |
| `Tab` | Автодополнение |
| `Up` / `Down` | История команд / навигация по подсказкам |
| `Ctrl+C` | Выход |
| `Ctrl+L` | Очистить логи |
| `Ctrl+U` | Очистить строку ввода |
| `Ctrl+W` | Удалить слово назад |
| `Ctrl+A` / `Ctrl+E` | Начало / конец строки |
| `Esc` | Закрыть подсказки |

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
 │   ├── tui.rs            -- TUI: ratatui, event loop, колоночный рендеринг
 │   ├── commands.rs       -- обработчики команд (вывод в буфер)
 │   └── completer.rs      -- автодополнение с историей пакетов/пресетов
 ├── logs/
 │   ├── parser.rs         -- парсер logcat (threadtime/brief)
 │   ├── filters.rs        -- composable-фильтры
 │   └── formatter.rs      -- конфигурация полей, пресеты
 ├── traffic/mod.rs        -- proxy-адаптер
 ├── mock/mod.rs           -- YAML rules engine
 └── config/mod.rs         -- пресеты + история приложений/фильтров
```

## Версии

| Тег | Язык | Описание |
|-----|------|----------|
| `v2.1.0` | Rust | TUI с колоночной раскладкой, ratatui |
| `v2.0.0` | Rust | Первая Rust-версия (rustyline REPL) |
| `v1.0.0-python` | Python | Предыдущая версия (prompt_toolkit + rich + mitmproxy) |

```bash
# Клонировать Python-версию
git clone --branch v1.0.0-python https://github.com/thothlab/logux.git
```

## Лицензия

MIT
