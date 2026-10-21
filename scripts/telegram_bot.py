#!/usr/bin/env python3
"""
Hydra-HFT v4.0 — Telegram Interactive Notification & Control Center
Connects to @HydraHFTbot, listens for user messages, and pushes instant
trade execution alerts directly to the user's phone.
"""

import sys
import time
import json
import os
import urllib.request
import urllib.parse

try:
    if sys.stdout:
        sys.stdout.reconfigure(encoding="utf-8")
    if sys.stderr:
        sys.stderr.reconfigure(encoding="utf-8")
except Exception:
    pass

BOT_TOKEN = "8851439624:AAFy-N0xi_mAGmi0mhzjAuUtDd_-MI956qM"
API_BASE = f"https://api.telegram.org/bot{BOT_TOKEN}"
CHAT_ID_FILE = os.path.join("logs", "tg_chat_id.txt")
CSV_LOG = os.path.join("logs", "paper_48h_150eur.csv")
STATUS_MD = os.path.join("logs", "paper_48h_live_status.md")


def call_api(method: str, params: dict = None) -> dict:
    url = f"{API_BASE}/{method}"
    headers = {"User-Agent": "Mozilla/5.0", "Content-Type": "application/json"}
    data = json.dumps(params).encode("utf-8") if params else None
    req = urllib.request.Request(url, data=data, headers=headers)
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            return json.loads(resp.read().decode("utf-8"))
    except Exception as e:
        return {"ok": False, "error": str(e)}


def send_message(chat_id: int, text: str, parse_mode: str = "Markdown") -> bool:
    payload = {
        "chat_id": chat_id,
        "text": text,
        "parse_mode": parse_mode,
        "disable_web_page_preview": True,
    }
    res = call_api("sendMessage", payload)
    return res.get("ok", False)


def get_stored_chat_ids() -> set:
    if os.path.exists(CHAT_ID_FILE):
        try:
            with open(CHAT_ID_FILE, "r", encoding="utf-8") as f:
                return {int(line.strip()) for line in f if line.strip().isdigit()}
        except Exception:
            pass
    return set()


def store_chat_id(chat_id: int):
    chats = get_stored_chat_ids()
    if chat_id not in chats:
        chats.add(chat_id)
        os.makedirs(os.path.dirname(CHAT_ID_FILE), exist_ok=True)
        with open(CHAT_ID_FILE, "a", encoding="utf-8") as f:
            f.write(f"{chat_id}\n")


def build_status_reply() -> str:
    if os.path.exists(STATUS_MD):
        try:
            with open(STATUS_MD, "r", encoding="utf-8") as f:
                content = f.read().strip()
                return f"📊 *ТЕКУЩИЙ СТАТУС HYDRA-HFT:*\n\n{content}"
        except Exception as e:
            return f"⚠️ Ошибка чтения статуса: {e}"
    return "⏳ Статус-файл еще формируется..."


def build_last_trades_reply(limit: int = 5) -> str:
    if not os.path.exists(CSV_LOG):
        return "⏳ Торговый лог пока пуст."
    try:
        trades = []
        with open(CSV_LOG, "r", encoding="utf-8") as f:
            for line in f:
                parts = line.strip().split(",")
                if len(parts) >= 12 and "FIRE" in parts[8]:
                    trades.append(parts)
        if not trades:
            return "ℹ️ Пока нет исполненных сделок FIRE (система ожидает импульса)."
        recent = trades[-limit:]
        msg = f"⚡ *ПОСЛЕДНИЕ {len(recent)} СДЕЛОК (FIRE):*\n\n"
        for t in reversed(recent):
            poll_idx, _, _, _, route, path, gross, net, action, notional, profit, equity = t[:12]
            msg += (
                f"• *Тик #{poll_idx}*: `[{route}]` {path}\n"
                f"  Спред: `{float(gross):+.2f}bps` (чистый: `{float(net):+.2f}bps`)\n"
                f"  Профит: *+€{float(profit):.4f}* | Баланс: *€{float(equity):.4f}*\n\n"
            )
        return msg
    except Exception as e:
        return f"⚠️ Ошибка чтения логов: {e}"


def run_telegram_service():
    print("[Telegram] Запуск сервиса оповещений Hydra-HFT...")
    last_update_id = 0
    known_chats = get_stored_chat_ids()
    last_processed_line = 0

    # Count initial lines in CSV
    if os.path.exists(CSV_LOG):
        with open(CSV_LOG, "r", encoding="utf-8") as f:
            last_processed_line = sum(1 for _ in f)

    while True:
        try:
            # 1. Process Telegram Updates
            params = {"offset": last_update_id + 1, "timeout": 2}
            res = call_api("getUpdates", params)
            if res.get("ok"):
                for update in res.get("result", []):
                    last_update_id = update["update_id"]
                    msg = update.get("message", {})
                    chat_id = msg.get("chat", {}).get("id")
                    text = msg.get("text", "").strip()
                    first_name = msg.get("from", {}).get("first_name", "User")

                    if not chat_id:
                        continue

                    if chat_id not in known_chats:
                        store_chat_id(chat_id)
                        known_chats.add(chat_id)
                        welcome = (
                            f"👋 Здравствуйте, *{first_name}*!\n\n"
                            f"🤖 *Hydra-HFT v4.0 Bot успешно подключен к вашему терминалу!*\n"
                            f"Вы будете мгновенно получать уведомления о каждой прибыльной сделке (FIRE).\n\n"
                            f"Команды:\n"
                            f"• /status — текущий статус системы и PnL\n"
                            f"• /last — последние совершенные сделки\n"
                            f"• /help — список команд"
                        )
                        send_message(chat_id, welcome)

                    if text in ("/start", "/help"):
                        help_text = (
                            f"🚀 *HYDRA-HFT КОМАНДНЫЙ ЦЕНТР*\n\n"
                            f"Доступные команды:\n"
                            f"• /status — статус, баланс, RTT и активные кольца\n"
                            f"• /last — история последних прибыльных сделок\n"
                            f"• /ping — проверка связи с ботом"
                        )
                        send_message(chat_id, help_text)
                    elif text == "/status":
                        send_message(chat_id, build_status_reply())
                    elif text == "/last":
                        send_message(chat_id, build_last_trades_reply(5))
                    elif text == "/ping":
                        send_message(chat_id, "🏓 Понг! Движок Hydra-HFT активен и отслеживает котировки.")

            # In live mode, notifications are sent exclusively by live_broadcaster.py 
            # upon confirmed on-chain transaction execution on Base Mainnet.
            pass
        except Exception as e:
            print(f"[Telegram Service Error] {e}")

        time.sleep(1.0)


if __name__ == "__main__":
    run_telegram_service()
