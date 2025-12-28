#!/usr/bin/env python3
"""
BTC Hunter Fleet Monitor Bot
Monitors heartbeat messages from mining devices and alerts when they go offline.
"""

import asyncio
import re
from datetime import datetime, timedelta
from collections import defaultdict
from telegram import Update
from telegram.ext import Application, CommandHandler, MessageHandler, filters, ContextTypes

# ========== CONFIGURATION ==========
TELEGRAM_TOKEN = "8567698385:AAF3JahzGZXWNKJd2i9IwY-2WRs2Qx9aiAI"
TELEGRAM_CHAT_ID = "888371592"

# Alert thresholds
STALE_THRESHOLD = timedelta(minutes=90)   # Warn after 90 min
DEAD_THRESHOLD = timedelta(hours=3)        # Mark dead after 3 hours
CHECK_INTERVAL = 30 * 60                   # Check every 30 minutes

# ========== STATE ==========
worker_last_seen = {}  # {worker_id: datetime}
worker_stats = defaultdict(dict)  # {worker_id: {speed, uptime, etc}}

# ========== HANDLERS ==========

async def handle_message(update: Update, context: ContextTypes.DEFAULT_TYPE):
    """Parse incoming messages for heartbeat patterns."""
    if not update.message or not update.message.text:
        return
    
    text = update.message.text
    
    # Pattern 1: Heartbeat messages
    heartbeat_match = re.search(r'💓 HEARTBEAT: (Worker-\w+)', text)
    if heartbeat_match:
        worker_id = heartbeat_match.group(1)
        worker_last_seen[worker_id] = datetime.now()
        
        # Extract stats if present
        speed_match = re.search(r'Speed: ([\d.]+) MKeys/s', text)
        uptime_match = re.search(r'Uptime: (\d+)h', text)
        
        if speed_match:
            worker_stats[worker_id]['speed'] = speed_match.group(1)
        if uptime_match:
            worker_stats[worker_id]['uptime'] = uptime_match.group(1)
        
        print(f"[✓] {worker_id} heartbeat received")
        return
    
    # Pattern 2: Startup messages
    startup_match = re.search(r'🚀 STARTED: (Worker-\w+)', text)
    if startup_match:
        worker_id = startup_match.group(1)
        worker_last_seen[worker_id] = datetime.now()
        print(f"[✓] {worker_id} came online")
        return


async def cmd_status(update: Update, context: ContextTypes.DEFAULT_TYPE):
    """Show fleet status dashboard."""
    if not worker_last_seen:
        await update.message.reply_text("📊 No workers registered yet.")
        return
    
    now = datetime.now()
    active = []
    stale = []
    dead = []
    
    for worker_id, last_seen in worker_last_seen.items():
        delta = now - last_seen
        minutes_ago = int(delta.total_seconds() / 60)
        
        stats = worker_stats.get(worker_id, {})
        speed = stats.get('speed', 'N/A')
        uptime = stats.get('uptime', 'N/A')
        
        info = f"{worker_id}\n  Last: {minutes_ago}m ago | Speed: {speed} MKeys/s | Uptime: {uptime}h"
        
        if delta < STALE_THRESHOLD:
            active.append(f"✅ {info}")
        elif delta < DEAD_THRESHOLD:
            stale.append(f"⚠️ {info}")
        else:
            dead.append(f"❌ {info}")
    
    report = "📊 **FLEET STATUS**\n\n"
    
    if active:
        report += "**ACTIVE:**\n" + "\n\n".join(active) + "\n\n"
    if stale:
        report += "**STALE:**\n" + "\n\n".join(stale) + "\n\n"
    if dead:
        report += "**DEAD:**\n" + "\n\n".join(dead) + "\n\n"
    
    report += f"**Summary:** {len(active)} active, {len(stale)} stale, {len(dead)} dead"
    
    await update.message.reply_text(report, parse_mode='Markdown')


async def cmd_workers(update: Update, context: ContextTypes.DEFAULT_TYPE):
    """List all known workers."""
    if not worker_last_seen:
        await update.message.reply_text("No workers registered.")
        return
    
    workers = sorted(worker_last_seen.keys())
    msg = f"👥 **Known Workers ({len(workers)}):**\n\n" + "\n".join(f"• {w}" for w in workers)
    await update.message.reply_text(msg, parse_mode='Markdown')


async def cmd_reset(update: Update, context: ContextTypes.DEFAULT_TYPE):
    """Clear worker history."""
    worker_last_seen.clear()
    worker_stats.clear()
    await update.message.reply_text("🔄 Worker history cleared.")


async def check_workers(context: ContextTypes.DEFAULT_TYPE):
    """Periodic check for offline workers."""
    now = datetime.now()
    
    for worker_id, last_seen in worker_last_seen.items():
        delta = now - last_seen
        
        # Send alert if worker just became stale (90-95 min range)
        if STALE_THRESHOLD <= delta < STALE_THRESHOLD + timedelta(minutes=5):
            minutes = int(delta.total_seconds() / 60)
            alert = (
                f"⚠️ **STALE WORKER DETECTED**\n\n"
                f"Worker: {worker_id}\n"
                f"Last Seen: {minutes} minutes ago\n"
                f"Status: Not responding"
            )
            await context.bot.send_message(chat_id=TELEGRAM_CHAT_ID, text=alert, parse_mode='Markdown')
            print(f"[!] Alert sent for {worker_id} (stale)")
        
        # Send alert if worker just became dead (3-3.1 hour range)
        elif DEAD_THRESHOLD <= delta < DEAD_THRESHOLD + timedelta(minutes=10):
            hours = delta.total_seconds() / 3600
            alert = (
                f"❌ **DEAD WORKER DETECTED**\n\n"
                f"Worker: {worker_id}\n"
                f"Last Seen: {hours:.1f} hours ago\n"
                f"Status: Presumed offline"
            )
            await context.bot.send_message(chat_id=TELEGRAM_CHAT_ID, text=alert, parse_mode='Markdown')
            print(f"[!] Alert sent for {worker_id} (dead)")


async def post_init(application: Application):
    """Schedule periodic worker checks."""
    job_queue = application.job_queue
    job_queue.run_repeating(check_workers, interval=CHECK_INTERVAL, first=CHECK_INTERVAL)
    print(f"[✓] Monitoring started (check every {CHECK_INTERVAL//60} minutes)")


def main():
    """Start the monitoring bot."""
    print("=" * 50)
    print("  BTC HUNTER FLEET MONITOR BOT")
    print("=" * 50)
    print(f"Chat ID: {TELEGRAM_CHAT_ID}")
    print(f"Stale Threshold: {STALE_THRESHOLD}")
    print(f"Dead Threshold: {DEAD_THRESHOLD}")
    print("=" * 50)
    
    application = Application.builder().token(TELEGRAM_TOKEN).post_init(post_init).build()
    
    # Command handlers
    application.add_handler(CommandHandler("status", cmd_status))
    application.add_handler(CommandHandler("workers", cmd_workers))
    application.add_handler(CommandHandler("reset", cmd_reset))
    
    # Message handler (parse heartbeats)
    application.add_handler(MessageHandler(filters.TEXT & ~filters.COMMAND, handle_message))
    
    print("[✓] Bot is running. Press Ctrl+C to stop.")
    application.run_polling(allowed_updates=Update.ALL_TYPES)


if __name__ == '__main__':
    main()
