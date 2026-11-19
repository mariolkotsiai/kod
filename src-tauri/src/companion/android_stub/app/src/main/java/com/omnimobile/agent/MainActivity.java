package com.omnimobile.agent;

import android.app.Activity;
import android.os.Bundle;
import android.view.MotionEvent;
import android.view.View;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.PrintWriter;
import java.net.ServerSocket;
import java.net.Socket;
import java.util.concurrent.atomic.AtomicBoolean;

/**
 * Ephemeral diagnostic stub activity for OmniMobile Toolkit.
 * Listens on local loopback socket (127.0.0.1:49152) during active session
 * and is uninstalled automatically via `adb shell pm uninstall com.omnimobile.agent`.
 */
public class MainActivity extends Activity {
    private static final int LOCAL_DIAG_PORT = 49152;
    private final AtomicBoolean running = new AtomicBoolean(false);
    private volatile int touchEventsObserved = 0;
    private ServerSocket serverSocket;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);

        View rootView = new View(this);
        rootView.setOnTouchListener((v, event) -> {
            if (event.getAction() == MotionEvent.ACTION_DOWN || event.getAction() == MotionEvent.ACTION_MOVE) {
                touchEventsObserved++;
            }
            return true;
        });
        setContentView(rootView);

        running.set(true);
        new Thread(this::serveLoopbackDiagnostics, "OmniDiagSocket").start();
    }

    private void serveLoopbackDiagnostics() {
        try {
            // Bind strictly to loopback interface (127.0.0.1) to prevent external network exposure.
            serverSocket = new ServerSocket(LOCAL_DIAG_PORT, 2, java.net.InetAddress.getByName("127.0.0.1"));
            while (running.get()) {
                try (Socket client = serverSocket.accept();
                     BufferedReader in = new BufferedReader(new InputStreamReader(client.getInputStream()));
                     PrintWriter out = new PrintWriter(client.getOutputStream(), true)) {
                    String cmd = in.readLine();
                    if ("PING_DIAG".equals(cmd)) {
                        out.println("OK|touch_events=" + touchEventsObserved);
                    }
                }
            }
        } catch (Exception ignored) {
            // Socket closes cleanly when activity or session terminates.
        }
    }

    @Override
    protected void onDestroy() {
        running.set(false);
        try {
            if (serverSocket != null && !serverSocket.isClosed()) {
                serverSocket.close();
            }
        } catch (Exception ignored) {
        }
        super.onDestroy();
    }
}