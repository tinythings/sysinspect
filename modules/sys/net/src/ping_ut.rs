#[cfg(test)]
mod tests {
    use crate::ping::{parse_ping_output, parse_rtt, parse_sent_received, parse_ttl};

    #[test]
    fn parse_linux_ping_output() {
        let out = r"PING 8.8.8.8 (8.8.8.8) 56(84) bytes of data.
64 bytes from 8.8.8.8: icmp_seq=1 ttl=118 time=10.5 ms
64 bytes from 8.8.8.8: icmp_seq=2 ttl=118 time=11.2 ms
64 bytes from 8.8.8.8: icmp_seq=3 ttl=118 time=12.1 ms

--- 8.8.8.8 ping statistics ---
3 packets transmitted, 3 received, 0% packet loss, time 2003ms
rtt min/avg/max/mdev = 10.500/11.266/12.100/0.655 ms";
        let stats = parse_ping_output(out, 3).unwrap();
        assert_eq!(stats.sent, 3);
        assert_eq!(stats.received, 3);
        assert_eq!(stats.loss_pct, 0.0);
        assert_eq!(stats.rtt_min, Some(10.5));
        assert_eq!(stats.rtt_avg, Some(11.266));
        assert_eq!(stats.rtt_max, Some(12.1));
        assert_eq!(stats.ttl, Some(118));
    }

    #[test]
    fn parse_loss() {
        let out = r"--- 8.8.8.8 ping statistics ---
3 packets transmitted, 1 received, 66.666% packet loss";
        let stats = parse_ping_output(out, 3).unwrap();
        assert_eq!(stats.sent, 3);
        assert_eq!(stats.received, 1);
        assert!((stats.loss_pct - 66.666).abs() < 1.0);
    }

    #[test]
    fn parse_rtt_values() {
        let (min, avg, max) = parse_rtt("rtt min/avg/max/mdev = 10.5/11.2/12.1/0.5 ms");
        assert_eq!(min, Some(10.5));
        assert_eq!(avg, Some(11.2));
        assert_eq!(max, Some(12.1));
    }

    #[test]
    fn parse_ttl_from_line() {
        assert_eq!(parse_ttl("64 bytes from 1.1.1.1: icmp_seq=1 ttl=118 time=10.5 ms"), Some(118));
    }

    #[test]
    fn parse_sent_received_stats() {
        assert_eq!(parse_sent_received("3 packets transmitted, 3 received, 0% packet loss", 3), Some(3));
        assert_eq!(parse_sent_received("5 packets transmitted, 0 received, 100% packet loss", 5), Some(0));
    }
}
