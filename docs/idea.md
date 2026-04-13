# 🧠 Android Logs & Traffic CLI — Техническое задание

Название утилиты 'logux' 
github: https://github.com/thothlab/logux.git

При выполнении

## 1. Цели

Разработать кроссплатформенную CLI-утилиту для Android-разработчиков, позволяющую:

* в реальном времени читать и анализировать логи приложения через ADB (`adb logcat`);
* быстро переключаться между фильтрами без перезапуска;
* получать структурированный, цветной и настраиваемый вывод логов;
* отслеживать сетевой трафик приложения;
* задавать правила подмены сетевых ответов через декларативный конфиг (YAML);
* управлять всем через интерактивную CLI с командами через `/`.

---

## 2. Scope (область реализации)

### Включено

* CLI shell с интерактивными командами
* Работа с ADB:

  * USB
  * TCP/IP (включая Wi-Fi подключение)
* Чтение логов **исключительно через `adb logcat`**
* Гибкая фильтрация логов
* Цветной вывод
* Presets
* Отдельный network-режим:

  * просмотр трафика
  * фильтрация
  * mock / rewrite rules через YAML
* Интеграция с proxy-engine (**вариант B**, например mitmproxy)

### Исключено (на текущем этапе)

* GUI
* Прямой перехват без proxy
* Гарантированная поддержка pinning/QUIC (best effort)

---

## 3. Основные сценарии использования

### Сценарий 1. Быстрый просмотр логов

* пользователь подключает устройство
* указывает package name
* автоматически применяется фильтр по приложению
* **в любой момент может изменить фильтр без перезапуска**

---

### Сценарий 2. Отладка через фильтры

* фильтрация по:

  * tag
  * level
  * regex
  * тексту
* изменение фильтров "на лету"

---

### Сценарий 3. Анализ трафика

* просмотр HTTP/HTTPS запросов
* фильтрация по:

  * host
  * path
  * method
  * status

---

### Сценарий 4. Подмена ответов

* все mock-правила описаны в одном YAML-файле
* каждое правило можно:

  * включить / выключить
  * отредактировать на лету
* поддержка условий:

  * URL
  * query
  * headers
  * body

---

## 4. Функциональные требования

---

## 4.1 CLI Shell

* команды через `/`
* автодополнение
* история команд
* подсказки параметров
* интерактивный режим (REPL)

---

## 4.2 Работа с ADB

### Поддержка:

* USB устройства
* TCP/IP устройства (`adb connect`)
* Wi-Fi (через adb over network)

### Возможности:

* список устройств
* выбор устройства
* переподключение
* отображение статуса

---

## 4.3 Источник логов

* только `adb logcat`
* запуск как subprocess
* потоковое чтение stdout

---

## 4.4 Фильтрация логов

### Поддерживаемые фильтры:

* package name (**по умолчанию включен**)
* PID (автообновляемый)
* tag
* level
* текст
* regex
* thread
* time range

### Особенность:

* **все фильтры можно менять "на лету" без перезапуска**

---

## 4.5 Smart-фильтр по приложению

* пользователь задает package name
* система:

  * находит PID
  * фильтрует по PID
  * при рестарте приложения:

    * автоматически находит новый PID
    * продолжает поток

---

## 4.6 Формат вывода

### Настраиваемые поля:

* timestamp
* level
* tag
* PID
* TID
* process
* thread
* message

### Presets:

* compact
* threadtime
* verbose
* minimal
* json

---

## 4.7 Цветной вывод

* level-based coloring
* выделение match'ей
* отдельный стиль для stacktrace

---

## 4.8 Работа с трафиком

### Архитектура

* proxy-based (вариант B)
* CLI управляет proxy engine

---

### Возможности

* список запросов
* просмотр request/response
* фильтрация:

  * host
  * path
  * method
  * status
* поиск по body

---

## 4.9 Mock / Rewrite rules

### Формат: YAML

Пример:

```yaml
rules:
  - id: user_profile_mock
    enabled: true

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

---

### Требования:

* загрузка одного YAML-файла
* hot reload
* включение/выключение правил на лету
* приоритет правил
* логирование примененных правил

---

## 5. Нефункциональные требования

* кроссплатформенность (macOS / Linux / Windows)
* высокая производительность (без лагов при большом потоке логов)
* минимальная задержка вывода
* устойчивость к разрывам ADB
* модульность (логика логов ≠ логика сети)

---

## 6. Архитектура

### Компоненты:

```
CLI Shell
   │
   ├── ADB Module
   │     └── logcat stream
   │
   ├── Log Pipeline
   │     ├── parser
   │     ├── filters
   │     ├── formatter
   │     └── renderer
   │
   ├── Traffic Module
   │     └── proxy adapter (mitmproxy-like)
   │
   └── Config / Presets
```

---

## 7. CLI команды

### Общие

```
/help
/exit
/clear
```

---

### ADB

```
/devices
/connect <device>
/disconnect
```

---

### Логи

```
/app <package>
/pid <pid>
/tag <tag>
/level <level>
/grep <text>
/regex <pattern>
/filter reset
```

---

### Формат

```
/format <preset>
/fields +timestamp -thread +tag
```

---

### Управление

```
/pause
/resume
/save <file>
```

---

### Presets

```
/preset save <name>
/preset load <name>
```

---

### Трафик

```
/traffic open
/traffic close
/traffic filter <expression>
```

---

### Mock

```
/mock load rules.yaml
/mock list
/mock enable <id>
/mock disable <id>
/mock reload
```

---

## 8. Roadmap

---

### MVP v1 — Logs (обязательный минимум)

* ADB подключение (USB + TCP/IP + Wi-Fi)
* logcat streaming
* фильтрация
* package-based smart filter
* цветной вывод
* presets
* CLI shell

---

### MVP v1.5 — Traffic наблюдение

* интеграция proxy
* просмотр запросов
* базовые фильтры
* inspect request/response

---

### MVP v2 — Mocking

* YAML rules
* enable/disable на лету
* reload
* response override
* latency injection
* forced errors




