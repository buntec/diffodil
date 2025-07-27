import { useRef, useEffect } from 'react'

export function useWebSocket(url: string, onMessage: any, onError: any, onClose: any) {
  const ws = useRef<WebSocket>(null);
  const heartbeatInterval = useRef<number>(null);

  useEffect(() => {
    function connect() {
      ws.current = new WebSocket(url);

      ws.current.onopen = () => {
        console.log("WebSocket connected");

        // Start heartbeat
        heartbeatInterval.current = setInterval(() => {
          if (ws.current && ws.current.readyState === WebSocket.OPEN) {
            ws.current.send(
              JSON.stringify({ type: "heartbeat", timestamp: Date.now() }),
            );
          }
        }, 10000);
      };

      ws.current.onmessage = (event) => {
        try {
          // console.info(`Received WS message: ${event.data}`)
          const data = JSON.parse(event.data);
          onMessage(data);
        } catch (error) {
          console.warn(`Failed to parse WS message to JSON: ${event.data}`)
        }
      };

      ws.current.onerror = (err) => {
        console.error("WebSocket error", err);
        onError(err);
      };

      ws.current.onclose = (event) => {
        console.log("WebSocket closed: ", event);
        console.log("Attempting to reconnect WebSocket...");
        onClose(event);
        setTimeout(() => connect(), 3000);
      };
    }

    // if we connect immediately, the first attempt fails (for the dev server)
    setTimeout(() => connect(), 500);

    return () => {
      if (ws.current) {
        ws.current.close();
      }
      if (heartbeatInterval.current) {
        clearInterval(heartbeatInterval.current);
      }
    };
  }, [url]);

  const sendMsg = (o: any) => {
    if (ws.current?.readyState === WebSocket.OPEN) {
      ws.current.send(JSON.stringify(o));
    }
  };

  return { sendMsg };
}

