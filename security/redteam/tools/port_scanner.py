#!/usr/bin/env python3
"""
Simple TCP port scanner for authorized lab auditing only.
"""
import argparse
import ipaddress
import socket
import sys
from concurrent.futures import ThreadPoolExecutor, as_completed

TOP_100 = [21, 22, 23, 25, 53, 80, 110, 111, 135, 139, 143, 443, 445, 993,
           995, 1723, 3306, 3389, 5432, 5900, 8080, 8443, 9200, 10000]


def scan_port(host: str, port: int, timeout: float = 1.0) -> tuple:
    try:
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
            s.settimeout(timeout)
            result = s.connect_ex((host, port))
            if result == 0:
                return (host, port, "open")
    except Exception:
        pass
    return (host, port, "closed")


def parse_targets(target: str):
    try:
        net = ipaddress.ip_network(target, strict=False)
        return [str(ip) for ip in net.hosts()]
    except ValueError:
        return [target]


def main():
    parser = argparse.ArgumentParser(description="TCP port scanner")
    parser.add_argument("target", help="IP, hostname or CIDR")
    parser.add_argument("-p", "--ports", type=str, help="Ports: 22,80,443 or 1-1000")
    parser.add_argument("--top-100", action="store_true", help="Scan top 100 ports")
    parser.add_argument("-t", "--threads", type=int, default=50, help="Threads")
    parser.add_argument("--timeout", type=float, default=1.0, help="Timeout seconds")
    args = parser.parse_args()

    if args.top_100:
        ports = TOP_100
    elif args.ports:
        if "-" in args.ports:
            start, end = map(int, args.ports.split("-"))
            ports = list(range(start, end + 1))
        else:
            ports = [int(p) for p in args.ports.split(",")]
    else:
        ports = [22, 80, 443]

    hosts = parse_targets(args.target)
    print(f"[*] Scanning {len(hosts)} host(s) on {len(ports)} port(s)")

    with ThreadPoolExecutor(max_workers=args.threads) as executor:
        futures = [executor.submit(scan_port, host, port, args.timeout)
                   for host in hosts for port in ports]
        for future in as_completed(futures):
            host, port, state = future.result()
            if state == "open":
                print(f"[+] {host}:{port} open")


if __name__ == "__main__":
    main()
