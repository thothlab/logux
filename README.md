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
- **Отслеживание приложения** -- автоматический PID-трекинг с обновлением при рестарте; последние фильтры автоматически восстанавливаются при повторном подключении
- **5 пресетов вывода** -- compact, threadtime, verbose, minimal, json
- **Авто-подключение** -- единственное устройство выбирается автоматически; при нескольких -- выбор из списка
- **Умное автодополнение** -- `/app` показывает историю пакетов и текущее foreground-приложение; `/filter` показывает пресеты, связанные с текущим приложением
- **Инспекция трафика** -- HTTP/HTTPS proxy через mitmproxy/mitmdump
- **Mock-правила** -- подмена ответов через YAML-конфигурацию с hot reload
- **Горячие клавиши** -- PageUp/Down прокрутка, Ctrl+C выход, Ctrl+L очистка, Tab автодополнение
- **Устойчивость стрима** -- битые UTF-8 байты (большие JSON от бэка logcat иногда режет посреди кодпоинта) больше не роняют поток; при обрыве `adb logcat` автоматически переподключается с backoff 0.5s → 10s (до 5 попыток). Если не помогло -- `/reconnect` полностью перезапускает adb server.

## Требования

- [Rust](https://rustup.rs/) 1.70+
- [ADB](https://developer.android.com/tools/adb) (Android Debug Bridge)
- [mitmproxy](https://mitmproxy.org/) (опционально, для инспекции трафика)

## Установка

### Вариант 1: скачать готовый бинарник (рекомендуется)

На странице [релизов](https://github.com/thothlab/logux/releases/latest) скачай архив под свою платформу, распакуй и положи бинарник в `$PATH`:

```bash
# macOS (Apple Silicon, M1/M2/M3/M4)
curl -L https://github.com/thothlab/logux/releases/latest/download/logux-macos-arm64.tar.gz | tar xz
sudo mv logux /usr/local/bin/
logux
```

```bash
# macOS (Intel)
curl -L https://github.com/thothlab/logux/releases/latest/download/logux-macos-x86_64.tar.gz | tar xz
sudo mv logux /usr/local/bin/
logux
```

Rust и `cargo` при этом не нужны. Требуется только установленный [ADB](https://developer.android.com/tools/adb).

> **macOS Gatekeeper:** если при первом запуске появится «не удалось проверить разработчика», выполни `xattr -d com.apple.quarantine $(which logux)` и запусти снова.

### Вариант 2: собрать из исходников

#### 1. Установить Rust (если ещё не установлен)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

#### 2. Собрать из исходников

```bash
git clone https://github.com/thothlab/logux.git
cd logux
cargo build --release
```

Бинарник: `./target/release/logux`

#### Установка в систему

```bash
cargo install --path .
```

После этого `logux` доступен из любой директории.

#### Обновление из исходников

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
│                                                      │ ← пустая строка-разделитель
│ 04-13 12:34:56  D  MyTag                             │ ← метаданные на первой строке
│     Short message                                    │ ← сообщение с отступом 4 пробела
│                                                      │
│ 04-13 12:34:57  W  NetworkManager                    │
│     This is a long message that wraps across         │ ← перенос по всей ширине
│     multiple lines keeping indentation               │
│ ...                                                  │
├──────────────────────────────────────────────────────┤
│  device_name   com.pkg   STREAMING         120 lines │ ← статус-бар
├──────────────────────────────────────────────────────┤
│ logux > /app mts_                                    │ ← строка ввода (всегда видна)
└──────────────────────────────────────────────────────┘
```

## Команды

### Общие

| Команда | Описание |
|---------|----------|
| `/help` | Справка по командам |
| `/exit` (alias: `/quit`) | Выход |
| `/clear` | Очистить экран |

### ADB

| Команда | Описание |
|---------|----------|
| `/devices` | Список устройств |
| `/connect <ip:port>` | Подключиться по TCP |
| `/disconnect` | Отключиться |
| `/reconnect` | Жёсткий перезапуск: `adb kill-server` + `start-server` + пересоздание потока логов. Полезно, если `adb` завис или авто-переподключение сдалось |

### Логи и фильтрация

| Команда | Описание |
|---------|----------|
| `/app <package>` | Фильтр по приложению (smart PID tracking) |
| `/pid <pid>` | Фильтр по PID |
| `/tag <tag>` | Добавить тег-фильтр (`-tag` убрать, `reset` сбросить) |
| `/level <V\|D\|I\|W\|E\|F>` | Минимальный уровень (`reset` сбросить) |
| `/grep <text>` | Поиск по тегу + сообщению (`reset` сбросить) |
| `/msg <text>` | Поиск только в сообщении (`-text` убрать, `reset` сбросить) |
| `/regex <pattern>` | Поиск по regex (`reset` сбросить) |
| `/exclude tag <name>` | Исключить тег из вывода |
| `/exclude msg <text>` | Исключить строки с текстом |
| `/exclude show` | Показать исключения |
| `/exclude reset` | Сбросить все исключения |
| `/exclude remove <value>` | Убрать одно исключение |
| `/filter` | Редактировать фильтры (= `/filter edit`) |
| `/filter show` | Показать активные фильтры |
| `/filter set <expr>` | Задать фильтры одной строкой |
| `/filter reset` | Сбросить все фильтры |
| `/filter <preset>` | Загрузить сохранённый пресет |
| `/forget` | Очистить все автосохранённые фильтры и историю по приложениям |

#### Как работают фильтры

**Ретроактивная фильтрация (как в Android Studio):** изменение любого фильтра мгновенно переприменяет фильтры ко всему буферу логов -- не только к новым строкам. Все ранее полученные записи хранятся в памяти и пересматриваются при обновлении фильтров. Работает точно как панель logcat в Android Studio.

**Все фильтры используют `contains` (частичное совпадение), не точное.**
Например, `tag=anal` покажет теги "Analytics", "AnalyticsTracker", "DataAnalysis".
Для точного совпадения используйте regex с якорями: `/regex ^Analytics$`.

**Включающие фильтры** комбинируются через **AND** (все условия одновременно):
```
/app ru.lewis.dbo    — по приложению
/tag network         — + тег содержит "network"
/level W             — + уровень >= WARN
/grep timeout        — + текст содержит "timeout"
```
Результат: показать только строки, где app=ru.lewis.dbo **И** tag содержит "network" **И** level >= W **И** текст содержит "timeout".

**OR внутри одного типа**: несколько тегов (`/tag A`, потом `/tag B`) работают как OR — строка проходит, если тег содержит A **ИЛИ** B.

**Исключающие фильтры** (аналог LogRabbit "None of the following"):
```
/exclude tag System.out       — скрыть тег содержащий "System.out"
/exclude tag CatalogParser    — скрыть ещё один
/exclude msg "[socket]:check" — скрыть строки с текстом
```

#### Редактирование фильтров

`/filter` или `/filter edit` загружает текущие фильтры в строку ввода:
```
logux > /filter set app=ru.lewis.dbo tag=network level=W !tag=System.out,Instana
```
Можно отредактировать и нажать Enter. Формат: `key=value` через пробел.

| Ключ | Описание |
|------|----------|
| `app=X` | Фильтр по приложению |
| `tag=A,B` | Теги (OR через запятую) |
| `level=W` | Минимальный уровень |
| `grep=text` | Текстовый поиск (тег + сообщение) |
| `msg=text` | Поиск только в сообщении (повторять для OR) |
| `regex=pattern` | Regex |
| `!tag=X,Y` | Исключить теги |
| `!msg=text` | Исключить по тексту |

**Автосохранение**: каждый `/filter set` автоматически сохраняется. При следующем `/filter` ранее использованные комбинации показываются как подсказки.

**Память фильтров по приложениям**: фильтры автоматически сохраняются для каждого пакета приложения. При повторном подключении к тому же приложению через `/app <package>` последние использованные фильтры (теги, уровень, grep, исключения) восстанавливаются автоматически.

### Формат

| Команда | Описание |
|---------|----------|
| `/format <preset>` | compact / threadtime / verbose / minimal / json |
| `/fields +field -field` | Включить/выключить поля: timestamp, level, tag, pid, tid |
| `/width <col>=<n> …` | Изменить ширину колонок: timestamp, level, tag, pid, tid |
| `/width show` / `/width reset` | Показать / сбросить ширины |
| `/copy [N]` | Скопировать в буфер последние N сообщений (по умолч. 50), только текст, без пробелов-колонок |

### Управление

| Команда | Описание |
|---------|----------|
| `/stop` | Полная остановка потока логов |
| `/pause` | Пауза/возобновление (toggle) |
| `/resume` | Возобновить вывод |
| `/save <file>` | Сохранять подходящие логи в файл (поддержка `~/`, без аргумента — остановить запись) |

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

### Горячие клавиши и прокрутка

| Клавиша | Действие |
|---------|----------|
| `Колёсико мыши` | Прокрутка логов по 1 строке (включено по умолчанию) |
| `Shift+Up` / `Shift+Down` | Прокрутка логов по 1 строке |
| `PageUp` / `PageDown` | Прокрутка логов по 10 строк |
| `Tab` | Автодополнение |
| `Up` / `Down` | История команд / навигация по подсказкам |
| `Shift+Enter` / `Alt+Enter` / `Ctrl+J` | Новая строка в поле ввода |
| `Ctrl+C` | Выход |
| `Ctrl+L` | Очистить логи |
| `Ctrl+U` | Очистить строку ввода |
| `Ctrl+W` | Удалить слово назад |
| `Ctrl+A` / `Ctrl+E` | Начало / конец строки |
| `Esc` | Закрыть подсказки |

**Прокрутка колёсиком мыши** включена по умолчанию. Для выделения и копирования текста удерживайте **Option/Alt** при перетаскивании (на macOS) или **Shift** (на Linux). Если предпочитаете нативное выделение без модификаторов — выполните `/mouse off`.

При прокрутке вверх стрим автоматически ставится на паузу. При возврате вниз до конца -- возобновляется. В статус-баре отображается `SCROLL +N` и подсказка `PageDown to resume`.

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
| `v2.1.0` | Rust | TUI с колоночной раскладкой, устойчивый стрим (UTF-8 lossy), авто-переподключение, `/reconnect` |
| `v2.0.0` | Rust | Первая Rust-версия (rustyline REPL) |
| `v1.0.0-python` | Python | Предыдущая версия (prompt_toolkit + rich + mitmproxy) |

```bash
# Клонировать Python-версию
git clone --branch v1.0.0-python https://github.com/thothlab/logux.git
```

## Лицензия

MIT
