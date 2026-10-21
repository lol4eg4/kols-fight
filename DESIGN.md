---
# gstack: design-md-format=spec
name: KOL Arena
description: Аркадный автомат для трейдеров. Тёмный зал, монетный жёлтый, неоновый розовый, пиксельные заголовки и чистые цифры.
colors:
  background: "#110d1c"
  surface: "#1a1430"
  surface-sunken: "#0e0b18"
  border: "#3a3160"
  text: "#f4f1ff"
  text-muted: "#a79fc6"
  coin: "#ffc93c"
  on-coin: "#110d1c"
  pink: "#ff4d8d"
  on-pink: "#110d1c"
  blue: "#6cc6ff"
  lavender: "#b79cff"
  plus: "#2fe5a7"
  minus: "#ff7062"
  shadow: "#05030a"
  podium-silver: "#d8d4ea"
  podium-bronze: "#f2a65a"
  tier-goat: "{colors.coin}"
  tier-diamond: "{colors.blue}"
  tier-sendoor: "{colors.pink}"
  tier-degen: "{colors.lavender}"
typography:
  display:
    fontFamily: Press Start 2P
    fontWeight: 400
    fontSize: clamp(2.25rem, 8vw, 4.5rem)
    lineHeight: 1.15
    letterSpacing: 0em
  section-title:
    fontFamily: Press Start 2P
    fontWeight: 400
    fontSize: clamp(1.125rem, 3.5vw, 1.75rem)
    lineHeight: 1.3
    letterSpacing: 0em
  pixel-label:
    fontFamily: Press Start 2P
    fontWeight: 400
    fontSize: 0.625rem
    lineHeight: 1.4
    letterSpacing: 0.02em
  title:
    fontFamily: Golos Text
    fontWeight: 600
    fontSize: 1.125rem
    lineHeight: 1.3
  body:
    fontFamily: Golos Text
    fontWeight: 400
    fontSize: 1rem
    lineHeight: 1.5
  small:
    fontFamily: Golos Text
    fontWeight: 400
    fontSize: 0.875rem
    lineHeight: 1.45
  label:
    fontFamily: JetBrains Mono
    fontWeight: 500
    fontSize: 0.75rem
    letterSpacing: 0.08em
  mono:
    fontFamily: JetBrains Mono
    fontWeight: 500
    fontSize: 0.9375rem
    fontFeature: tnum
  stat:
    fontFamily: JetBrains Mono
    fontWeight: 700
    fontSize: clamp(1.5rem, 4vw, 2rem)
    fontFeature: tnum
rounded:
  sm: 4px
  md: 8px
  lg: 12px
  full: 9999px
spacing:
  2xs: 4px
  xs: 8px
  sm: 12px
  md: 16px
  lg: 24px
  xl: 32px
  2xl: 48px
  3xl: 64px
components:
  button-primary:
    backgroundColor: "{colors.coin}"
    textColor: "{colors.on-coin}"
    fontFamily: "{typography.pixel-label.fontFamily}"
    fontSize: "{typography.pixel-label.fontSize}"
    rounded: "{rounded.sm}"
    padding: 16px 24px
  button-fight:
    backgroundColor: "{colors.pink}"
    textColor: "{colors.on-pink}"
    fontFamily: "{typography.pixel-label.fontFamily}"
    fontSize: "{typography.pixel-label.fontSize}"
    rounded: "{rounded.sm}"
    padding: 16px 24px
  button-secondary:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    borderColor: "{colors.border}"
    fontFamily: "{typography.pixel-label.fontFamily}"
    fontSize: "{typography.pixel-label.fontSize}"
    rounded: "{rounded.sm}"
    padding: 16px 20px
  input:
    backgroundColor: "{colors.surface-sunken}"
    textColor: "{colors.text}"
    borderColor: "{colors.border}"
    fontFamily: "{typography.body.fontFamily}"
    fontSize: "{typography.body.fontSize}"
    rounded: "{rounded.sm}"
    padding: 12px 16px
  card:
    backgroundColor: "{colors.surface}"
    borderColor: "{colors.border}"
    rounded: "{rounded.md}"
    padding: 20px
  stat-tile:
    backgroundColor: "{colors.surface}"
    borderColor: "{colors.border}"
    rounded: "{rounded.md}"
    padding: 20px
  modal:
    backgroundColor: "{colors.surface}"
    borderColor: "{colors.coin}"
    rounded: "{rounded.lg}"
    padding: 24px
  tier-chip:
    fontFamily: "{typography.pixel-label.fontFamily}"
    fontSize: "{typography.pixel-label.fontSize}"
    rounded: "{rounded.sm}"
    padding: 4px 8px
  network-chip:
    backgroundColor: "{colors.surface-sunken}"
    textColor: "{colors.text}"
    borderColor: "{colors.text-muted}"
    fontFamily: "{typography.small.fontFamily}"
    fontSize: "{typography.small.fontSize}"
    rounded: "{rounded.full}"
    padding: 2px 10px
  filter-pill:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    borderColor: "{colors.border}"
    rounded: "{rounded.full}"
    padding: 8px 14px
  filter-pill-active:
    backgroundColor: "{colors.coin}"
    textColor: "{colors.on-coin}"
  table-row:
    backgroundColor: "{colors.surface}"
    borderColor: "{colors.border}"
    padding: 12px 16px
  score-bar:
    backgroundColor: "{colors.surface-sunken}"
    height: 8px
  hp-segment:
    backgroundColor: "{colors.plus}"
    height: 8px
  ticker:
    backgroundColor: "{colors.surface-sunken}"
    textColor: "{colors.text}"
    fontFamily: "{typography.label.fontFamily}"
    fontSize: "{typography.label.fontSize}"
---

# KOL Arena

## Overview

**Северная звезда:** аркадный автомат для трейдеров. Страница играет, цифры остаются честными и читаются сразу.

**Продукт:** клиентская страница, где проекты на Solana выбирают KOL по реальным результатам кошельков. Рейтинг, карточка KOL, бой KOL Fight, игры без денег. Плюс закрытый раздел команды: сеть, кошельки, подбор, состояние сбора.

**Режимы экранов:**

- Главная: убедить и увлечь. Hero, пьедестал, лидерборд, бой.
- Карточка KOL и окно боя: опыт. Здесь главный момент продукта.
- Закрытый раздел команды: работа. Плотные списки и формы, тот же стиль, меньше игры.

**Источник:** прототип владельца и его 6 скриншотов, требования, раздел 6. Владелец решил 19 октября 2026: дизайн-система собирается из прототипа, нового стиля нет.

**Первые пять секунд:**

- тёмный зал и жёлтый пиксельный заголовок KOL ARENA с розовой тенью;
- пиксельные аватары-спрайты у каждого KOL;
- жёсткие тени без размытия у кнопок и карточек;
- зелёные и красные суммы моноширинным шрифтом;
- бегущая строка сверху.

## Colors

**Стратегия:** полная палитра. Тёмная основа, монетный жёлтый ведёт, розовый, голубой и лавандовый отличают тиры и бой, зелёный и красный — только для денег.

**Только тёмная тема.** Аркадный зал тёмный по замыслу; светлой темы нет.

Правила:

- Жёлтый (`coin`) — главное действие, лидер, победитель раунда, фокус. На экране одна жёлтая кнопка.
- Розовый (`pink`) — бой: кнопка FIGHT, заголовок KOL FIGHT, K.O., тень заголовка KOL ARENA.
- Цвет тира: GOAT — жёлтый, Diamond — голубой, Sendoor — розовый, Degen — лавандовый. Полоса Score берёт цвет тира.
- `plus` и `minus` — только суммы и изменения. Знак «+» или «−» стоит всегда: цвет не единственный носитель смысла.
- `text-muted` — подписи, источники, даты, «—» без данных.
- Нейтральные цвета имеют фиолетовый оттенок основы, чистого серого нет.
- Контраст: основной текст и `text-muted` на фоне и панели — от 4.5:1. Текст на жёлтой и розовой кнопке тёмный (`on-coin`, `on-pink`).

## Typography

Три шрифта из прототипа, у каждого своя роль:

- **Press Start 2P** — голос автомата. Заголовок KOL ARENA, заголовки разделов, кнопки, значки тира и места. Не для абзацев и не для цифр в таблице.
- **Golos Text** — текст: имена KOL, описания, формы, подсказки. Поддерживает кириллицу, русская версия ляжет без замены шрифта.
- **JetBrains Mono** — цифры и служебные подписи: суммы, проценты, Score, время, адрес кошелька, заголовки колонок, бегущая строка. Цифры табличные (`tnum`), столбцы выравниваются.

Загрузка: файлы шрифтов лежат на нашем сервере (пакеты `@fontsource/press-start-2p`, `@fontsource/golos-text`, `@fontsource/jetbrains-mono`, лицензия OFL), латиница и кириллица. Запросов к Google нет. `font-display: swap`.

Масштаб: заголовок hero до 4.5rem, заголовок раздела до 1.75rem, текст 1rem, подписи 0.75rem. Пиксельный шрифт в кнопках и значках — 0.625rem, всегда заглавными.

Заголовок KOL ARENA: жёлтый, жёсткая розовая тень со сдвигом вправо и вниз на 4px, без размытия.

## Layout

- Ширина содержимого до 1148px, по центру. Поля: 16px на телефоне, 24px на планшете, 32px шире.
- Телефон 360–759px: одна колонка. Строки лидерборда — карточки. Карточка KOL и окно боя на весь экран. Горизонтальной прокрутки нет.
- Планшет 760–1099px: плашки статистики по две в ряд, пьедестал в ряд, таблица со скрытыми второстепенными колонками.
- От 1100px: всё как на скриншотах — 4 плашки статистики, 3 места пьедестала, полная таблица.
- Ритм: внутри блока шаги 8–16px, между разделами 64px.
- Шапка закреплена сверху: логотип, ссылки на разделы, позже переключатель языка.
- Закрытый раздел: тот же каркас, слева меню из 6 пунктов, на телефоне меню сверху.

## Elevation & Depth

- Глубина — жёсткая тень без размытия: `4px 4px 0` цвета `shadow`. У кнопок, плашек пьедестала и бойцов.
- Нажатая кнопка смещается на 2px вниз и вправо, тень уменьшается до 2px.
- Слои отличаются фоном: `background` → `surface` → `surface-sunken` для ячеек внутри панели.
- Без свечения, без размытых теней, без матового стекла.
- Окно поверх страницы затемняет её: `background` с прозрачностью 80%.

## Shapes

- `sm` 4px — кнопки, поля, значки тира.
- `md` 8px — панели, плашки, строки-карточки на телефоне.
- `lg` 12px — карточка KOL и окно боя.
- `full` — фильтры тиров, значок «сеть», переключатель языка.
- Пиксельный аватар — квадрат 5×5 клеток, зеркальный по вертикали, один цвет палитры на тёмной плитке `surface-sunken`. Рисунок и цвет выводятся из id KOL, у одного KOL всегда одинаковы. Внешней картинки нет.

## Components

- **Кнопка главная:** жёлтая, тёмный пиксельный текст, жёсткая тень. Наведение: светлее на 8%. Фокус: жёлтая рамка 2px со сдвигом 2px. Выключена: прозрачность 40%, без тени.
- **Кнопка FIGHT:** как главная, но розовая. Только запуск боя.
- **Кнопка второстепенная:** панель с рамкой `border`, светлый пиксельный текст, жёсткая тень.
- **Значок тира:** рамка и текст цвета тира, фон — цвет тира с прозрачностью 12%, иконка и название. Пример: 💎 DIAMOND.
- **Значок «сеть»:** таблетка с 🤝 и словом, рядом с именем KOL сети.
- **Значки KOL:** эмодзи из требований (🎯 🔥 🐋 ⚡ 🙌 🟢 📡). Подсказка с объяснением — по наведению и по фокусу.
- **Плашка статистики:** большая цифра JetBrains Mono, подпись `text-muted` ниже.
- **Пьедестал:** три плашки; первое место в центре, выше остальных, жёлтая рамка; второе — серебряная, третье — бронзовая. Значок места пиксельным шрифтом.
- **Строка лидерборда:** место и изменение места, аватар, имя и @хэндл, значок тира, Score с полосой, PnL 30, 7 дней и 24 часа, win rate, позиция, последняя сделка, кнопка ⚔️ «в бой». Наведение — фон светлее. Вся строка открывает карточку KOL.
- **Полоса Score:** 8px, фон `surface-sunken`, заполнение цветом тира на долю от 100.
- **Разбор Score:** четыре строки: часть, сырое значение, доля шкалы полосой, очки. Внизу сумма и итог.
- **Нет данных:** «—» цвета `text-muted`, причина в подсказке. Никогда не ноль.
- **Устаревшие данные:** значок «данные устарели» с датой снимка рядом с цифрами.
- **Слот бойца:** плашка с аватаром, именем, значками и Score; рамка розовая. Пустой слот — пунктирная рамка и выбор KOL с поиском.
- **Полоса HP:** 8 сегментов, по одному на раунд, зелёные; потерянный сегмент — `surface-sunken`.
- **Строка раунда:** номер и название раунда, строка «что меряем», полосы бойцов. Победитель — жёлтая полоса и ★, остальные — лавандовые. Не засчитанный раунд — прозрачность 50% и подпись «не засчитан: не у всех есть данные».
- **Финал K.O.:** «K.O.!» розовым пиксельным шрифтом, имя победителя, счёт, 2–3 строки сводки; рамка жёлтая.
- **Бегущая строка:** моноширинный текст на `surface-sunken`, медленная прокрутка; при наведении и фокусе останавливается.
- **Поле:** фон `surface-sunken`, рамка `border`; фокус — рамка `coin`. Ошибка — рамка `minus` и текст ошибки под полем.
- **Пустое состояние:** одна строка, что случилось и что сделать. Без картинок.
- **Загрузка:** серые полосы на месте строк. Без крутящихся значков.
- **Закрытый раздел:** те же поля, кнопки и списки. Значки статуса кошелька: подтверждён — `plus`, гипотеза — `coin`, не найден — `text-muted`; всегда со словом.

## Do's and Don'ts

- Do: каждая сумма — JetBrains Mono, со знаком и цветом `plus` или `minus`.
- Do: у каждой цифры рядом или в подсказке — источник и дата снимка.
- Do: кнопка и любая цель касания на телефоне — от 44×44px.
- Do: видимый фокус у всего, что нажимается; окно держит фокус внутри, Esc закрывает.
- Do: проверка 360px без горизонтальной прокрутки на каждом экране.
- Don't: Press Start 2P в абзацах, таблицах цифр и формах.
- Don't: размытые тени, свечение, градиентные кнопки, стекло.
- Don't: новые цвета вне палитры; зелёный и красный — не для украшения.
- Don't: цвет как единственный признак: тир, победа и статус всегда подписаны словом или знаком.
- Don't: ноль вместо пустых данных.

## Motion

- **Подход:** осмысленное движение. Анимируется только то, что объясняет результат.
- **Плавность:** появление ease-out, уход ease-in, перемещение ease-in-out.
- **Длительность:** нажатие 80ms, переходы 200ms, заполнение полос 400ms, удар HP 250ms.
- **Главный момент — бой.** Экран VS, затем раунды по одному: полосы заполняются, победитель получает ★, проигравшие теряют сегмент HP. В конце штамп «K.O.!». Кнопки «Next» и «Skip animation» всегда видны.
- **Без анимации, если она отключена в системе** (`prefers-reduced-motion`): раунды переключаются кнопкой «Next» без движения, бегущая строка стоит, полосы сразу полные.

## Decisions Log

- 19 октября 2026 — дизайн-система собрана из прототипа владельца. Цвета и шрифты — из требований, раздел 6; размеры, формы и тени — со скриншотов.
- 19 октября 2026 — только тёмная тема, как в прототипе.
- 19 октября 2026 — шрифты лежат на нашем сервере, без запросов к Google.
- 19 октября 2026 — текст на жёлтой и розовой кнопке тёмный, ради контраста.
