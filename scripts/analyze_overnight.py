import csv

total_lines = 0
fire_count = 0
total_pnl = 0.0
path_stats = {}
route_stats = {}
profits = []

with open('logs/paper_48h_150eur.csv', 'r', encoding='utf-8') as f:
    reader = csv.DictReader(f)
    for row in reader:
        total_lines += 1
        action = row.get('action', '')
        if 'FIRE' in action:
            fire_count += 1
            pnl = float(row.get('net_profit_eur', 0.0))
            profits.append(pnl)
            total_pnl += pnl
            path = row.get('best_path', 'unknown')
            route = row.get('best_route', 'unknown')
            
            path_stats[path] = path_stats.get(path, 0.0) + pnl
            route_stats[route] = route_stats.get(route, 0) + 1

print(f"Total Polls: {total_lines}")
print(f"FIRE Trades: {fire_count} (Win Rate / Execution Ratio: {fire_count/total_lines*100:.2f}%)")
print(f"Total Net PnL: €{total_pnl:.4f}")
if profits:
    print(f"Average Profit per Trade: €{total_pnl / fire_count:.4f}")
    print(f"Max Single Trade Profit: €{max(profits):.4f}")
    print(f"Min Single Trade Profit: €{min(profits):.4f}")

print("\n--- TOP PROFITABLE ARBITRAGE PATHS ---")
sorted_paths = sorted(path_stats.items(), key=lambda x: x[1], reverse=True)[:5]
for p, s in sorted_paths:
    print(f"{p}: +€{s:.2f}")

print("\n--- TOP MOST ACTIVE VENUE ROUTES ---")
sorted_routes = sorted(route_stats.items(), key=lambda x: x[1], reverse=True)[:5]
for r, c in sorted_routes:
    print(f"{r}: {c} executions")
