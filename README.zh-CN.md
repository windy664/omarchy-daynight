# omarchy-daynight

让 Omarchy 主题跟着**真实日出日落**自动切换，而不是写死一个钟点。

Omarchy 本身没有这个功能：`hyprsunset` 只能在你自己填写的固定时间调整屏幕色温，
主题切换器则完全靠手动。这个项目加了一个定时器，按你的坐标算出当天的日出日落，
在白天主题和夜晚主题之间自动切换。

- **零依赖**：只用 Python 3 标准库，不需要 `pip install`，运行时不联网。
- **够准**：与 sunrise-sunset.org 的官方接口相比，五个城市 × 四个季节，最大偏差 0.36 分钟。
- **能自愈**：错过了日出（比如笔记本睡着了）？下一次检查会自动补上。
- **对国内友好**：`locate` 通过中国气象局定位，无需 API key、无需梯子。

---

## 为什么会有这个项目

大多数人都是自己手搓一版：两个 `systemd --user` 定时器，07:00 切亮色主题、
19:00 切暗色主题。第一天是好的，然后某天它悄悄地不工作了——原因值得说清楚。

### 所有手写版本都会踩的坑

```
11月 15 09:41:10.169 omarchy systemd[902]: Starting 白天主题：Catppuccin Latte...
11月 15 09:41:10.199 omarchy omarchy[914]: Theme 'catppuccin-latte' does not exist
11月 15 09:41:10.199 omarchy systemd[902]: theme-day.service: Main process exited, code=exited, status=1/FAILURE
```

`omarchy-theme-set` 解析主题名的方式是：

```bash
OMARCHY_THEMES_PATH="$OMARCHY_PATH/themes"
if [[ ! -d $OMARCHY_THEMES_PATH/$THEME_NAME ]] && [[ ! -d $USER_THEMES_PATH/$THEME_NAME ]]; then
  echo "Theme '$THEME_NAME' does not exist"
  exit 1
fi
```

而 `$OMARCHY_PATH` 在 systemd **user manager** 刚启动时并不在环境里。它要晚一点
才由 uwsm 的环境预加载器注入（图形会话起来的时候）：

```
09:41:10.121  systemd[902]: Started 每天 07:00 切亮色主题     <- 定时器激活
09:41:10.169  systemd[902]: Starting 白天主题                  <- 开始执行
09:41:10.199  omarchy[914]: Theme 'catppuccin-latte' does not exist
09:41:10.668  uwsm[941]: Selected compositor ID: hyprland.desktop <- 环境迟到 500ms
```

带 `Persistent=true` 的定时器会在**激活的那一刻**立刻补跑错过的日历事件，
而这恰好就是 manager 刚启动的时候。于是补跑每次都输掉这场竞争：
`$OMARCHY_PATH` 还是空的，路径变成 `/themes`，主题被报告为"不存在"。

由此得到两条规律：

1. **准点**执行（07:00:00 且机器醒着）完全正常——那时 uwsm 早就把环境导入了。
2. **开机后或唤醒后**补跑则每次都失败。

所以这个故障看起来"随机"：它只发生在机器在切换时刻处于关机的那些天。

修复只要在 unit 里加一行：

```ini
[Service]
Environment=OMARCHY_PATH=/usr/share/omarchy
```

本项目的 unit 里就带着这行，脚本内部也会自己补一次，所以无论怎么启动都不会再踩。

---

## 安装

```bash
git clone https://github.com/windy664/omarchy-daynight
cd omarchy-daynight
./install.sh
```

`install.sh` 会写配置、装脚本、放好 systemd user unit 并启用定时器，
过程中会自动尝试定位你的坐标。

如果你已经有自己写的 `theme-day.timer` / `theme-night.timer`，可以顺手一起停掉：

```bash
./install.sh --migrate-legacy
```

在你自己机器上的仓库克隆里执行：

```bash
./bin/omarchy-daynight install
```

### 依赖

- Omarchy（切主题走 `omarchy theme set`）
- Python 3.9+（需要 `zoneinfo`）
- systemd --user 定时器，Omarchy 本来就有

---

## 配置

`~/.config/omarchy-daynight/config`

```ini
# 日出到日落之间用这个主题，日落到次日日出之间用另一个。
day_theme=catppuccin-latte
night_theme=catppuccin

# 你的位置。install 会帮你填好。
latitude=22.97
longitude=113.74
# 留空表示跟随系统时区。
timezone=Asia/Shanghai

# 切换时刻偏移（分钟）：负数提前，正数推后。
dawn_offset=0
dusk_offset=0

# 定时器多久检查一次相位变化。
poll_interval=10min

# 仅在极昼极夜地区使用（太阳可能整天不升或不落）。
fallback_day_start=07:00
fallback_night_start=19:00

# 夜灯（hyprsunset）联动：off 或 auto。
nightlight=off

# 每次切主题后额外执行的命令。
on_switch=
```

任何 Omarchy 认识的主题都行，用 `omarchy theme list` 查看。Catppuccin 这对
（`catppuccin-latte` 和 `catppuccin`）是 Omarchy 自带的；`flexoki-light` 和
`tokyo-night` 也是不错的选择。

### 偏移量

`dawn_offset` / `dusk_offset` 把切换时刻从"太阳压地平线的那一刻"挪开。
`dawn_offset=-15` 表示在日出前 15 分钟就切成亮色主题，通常比"太阳刚出来才切"
体验更好——那时候屏幕还很暗。

### 暂停

```bash
touch ~/.local/state/omarchy-daynight/pause   # 暂停切换
rm ~/.local/state/omarchy-daynight/pause       # 恢复
```

---

## 命令

```bash
omarchy-daynight status     # 今天日出日落、当前主题、期望主题
omarchy-daynight run        # 相位变了就切
omarchy-daynight locate     # 根据公网 IP 猜出你的坐标
omarchy-daynight install    # 安装并启用
omarchy-daynight uninstall  # 移除 unit，保留配置
omarchy-daynight selftest   # 自检太阳计算
```

`status` 输出示例：

```
location   22.97, 113.74 (Asia/Shanghai)
sunrise    2026-11-15T06:38:08+08:00
sunset     2026-11-15T17:40:52+08:00
phase      morning
theme now  catppuccin-latte
theme want catppuccin-latte
timer      active
```

---

## 工作原理

日出日落每天都会漂几分钟，所以固定时刻的日历定时器不可能一直准。
这个项目改成**轮询**：

1. 定时器每 `poll_interval`（默认 10 分钟）醒一次。
2. 脚本按你的坐标算出当天的日出和日落。
3. 判断当前属于"白天"还是"夜晚"。
4. 和 Omarchy 当前主题不一致才切换，一致就什么都不做。

轮询的另一个好处是：错过的日出会在下一次检查时补上，而不是永远丢失。
每次切换前都会先比对，所以状态正确时完全静默。

计算用的是 NOAA 太阳位置算法，并且**在太阳过中天时刻**取值，而不是当地午夜。
这个细节很关键：赤纬若按午夜取值，高纬度地区昼长会差两分钟，且越靠近春分
秋分误差越大。

极昼极夜会被识别出来，退回到 `fallback_day_start` / `fallback_night_start`。

### 精度

与 sunrise-sunset.org 接口对比，五个城市（东莞、北京、纽约、悉尼、雷克雅未克）
× 四个季节，共 22 组：

| | 最大偏差 |
|---|---|
| 日出 | 0.36 分钟 |
| 日落 | 0.36 分钟 |

测试套件还会拿 `astral`（同算法的另一份独立实现）和接口给出的昼长做交叉校验：

```bash
python3 test/solar_test.py
```

---

## 怎么拿到自己的坐标

`omarchy-daynight locate` 用两个在国内无需梯子、无需 API key 就能访问的服务：

1. `myip.ipip.net` 报出你所在的城市；
2. `weather.cma.cn`（中国气象局）把城市换成气象站，站号信息里带经纬度和时区。

想让每次运行都刷新坐标就设 `auto_locate=1`；更省事的做法是直接手填
`latitude` / `longitude` 然后不动它——太阳在相邻城镇之间差别不大。

不在国内的话跳过这一步，自己填坐标即可。

---

## 夜灯

Omarchy 的 `hyprsunset` 默认是关的，因为它会给屏幕加一层色偏。
把 `nightlight=auto` 打开，本项目就会在日落时开夜灯、日出时关掉，
用的是菜单里同一个 `omarchy toggle nightlight` 开关。

---

## 卸载

```bash
omarchy-daynight uninstall
```

会停掉定时器并删除 unit，配置文件保留。

---

## 说明

- 切主题走 `omarchy theme set`，所以 Omarchy 自带的所有主题钩子照常执行：
  GTK 主题会跟着变、浏览器策略会跟着变、终端和编辑器也会跟着变。
- `on_switch` 会在每次切主题后执行一条命令，可以用它来轮换壁纸。
- 这些都是 user unit，不需要 root，重启后依然有效。

## 许可

MIT，见 [LICENSE](LICENSE)。
