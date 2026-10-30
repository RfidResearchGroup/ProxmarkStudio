package app.tauri.transport

import android.Manifest
import android.app.Activity
import android.app.PendingIntent
import android.bluetooth.BluetoothAdapter
import android.bluetooth.BluetoothClass
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothManager
import android.bluetooth.BluetoothSocket
import android.content.BroadcastReceiver
import android.content.ClipData
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageManager
import android.hardware.usb.UsbConstants
import android.hardware.usb.UsbDevice
import android.hardware.usb.UsbDeviceConnection
import android.hardware.usb.UsbEndpoint
import android.hardware.usb.UsbInterface
import android.hardware.usb.UsbManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.Base64
import android.util.Log
import androidx.core.content.FileProvider
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.InputStream
import java.io.OutputStream
import java.io.File
import java.util.UUID
import java.util.LinkedHashMap
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean

private const val TAG = "Pm3Transport"
private const val ACTION_USB_PERMISSION = "app.tauri.transport.USB_PERMISSION"
private val SPP_UUID: UUID = UUID.fromString("00001101-0000-1000-8000-00805F9B34FB")

@InvokeArg
class OpenUsbArgs {
    lateinit var path: String
    var baudRate: Int = 115200
}

@InvokeArg
class ShareFileArgs {
    lateinit var path: String
    var mime: String = "application/octet-stream"
    var name: String = ""
}

@InvokeArg
class OpenSppArgs {
    lateinit var address: String
}

@InvokeArg
class WriteArgs {
    lateinit var data: String
}

@TauriPlugin
class TransportPlugin(private val activity: Activity) : Plugin(activity) {
    private val usbManager: UsbManager
        get() = activity.getSystemService(Context.USB_SERVICE) as UsbManager

    private var pendingInvoke: Invoke? = null
    private var pendingDevice: UsbDevice? = null
    private var pendingBaud = 115200

    @Volatile private var closed = false
    private var usbConnection: UsbDeviceConnection? = null
    private var usbIn: UsbEndpoint? = null
    private var usbOut: UsbEndpoint? = null
    private var usbReader: Thread? = null

    private var sppSocket: BluetoothSocket? = null
    private var sppInput: InputStream? = null
    private var sppOutput: OutputStream? = null
    private var sppReader: Thread? = null

    private val permissionReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            if (intent.action != ACTION_USB_PERMISSION) return
            val invoke = pendingInvoke ?: return
            val device = pendingDevice
            pendingInvoke = null
            pendingDevice = null
            if (device != null && usbManager.hasPermission(device)) {
                try {
                    openUsbInternal(device, pendingBaud)
                    invoke.resolve()
                } catch (error: Exception) {
                    invoke.reject(error.message ?: "Failed to open USB serial device")
                }
            } else {
                invoke.reject("USB permission was not granted")
            }
        }
    }

    override fun load(webView: android.webkit.WebView) {
        super.load(webView)
        val filter = IntentFilter(ACTION_USB_PERMISSION)
        if (Build.VERSION.SDK_INT >= 33) {
            activity.registerReceiver(permissionReceiver, filter, Context.RECEIVER_EXPORTED)
        } else {
            @Suppress("DEPRECATION")
            activity.registerReceiver(permissionReceiver, filter)
        }
    }

    override fun onDestroy() {
        closeAll()
        try {
            activity.unregisterReceiver(permissionReceiver)
        } catch (_: Exception) {
        }
        super.onDestroy()
    }

    @Command
    fun shareFile(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(ShareFileArgs::class.java)
            val source = File(args.path)
            if (!source.isFile) return invoke.reject("The file does not exist.")
            val safeName = (args.name.ifBlank { source.name })
                .replace(Regex("[\\\\/:*?\"<>|]"), "_")
            val shareDir = File(activity.cacheDir, "shared").apply { mkdirs() }
            val target = File(shareDir, safeName)
            source.copyTo(target, overwrite = true)
            val uri = FileProvider.getUriForFile(
                activity,
                "${activity.packageName}.fileprovider",
                target,
            )
            val send = Intent(Intent.ACTION_SEND).apply {
                type = args.mime.ifBlank { "application/octet-stream" }
                putExtra(Intent.EXTRA_STREAM, uri)
                clipData = ClipData.newUri(activity.contentResolver, "Proxmark Studio file", uri)
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            }
            val chooser = Intent.createChooser(send, "Share").apply {
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            }
            activity.startActivity(chooser)
            invoke.resolve()
        } catch (error: Exception) {
            invoke.reject(error.message ?: "Could not share the file")
        }
    }

    @Command
    fun listUsb(invoke: Invoke) {
        try {
            val devices = JSArray()
            for (device in usbManager.deviceList.values) {
                val cdc = (0 until device.interfaceCount).any { index ->
                    val intf = device.getInterface(index)
                    intf.interfaceClass == UsbConstants.USB_CLASS_COMM ||
                        intf.interfaceClass == UsbConstants.USB_CLASS_CDC_DATA
                }
                if (!cdc) continue
                val item = JSObject()
                item.put("path", device.deviceName)
                item.put("name", device.productName ?: device.deviceName)
                item.put("manufacturer", device.manufacturerName ?: "")
                item.put("vid", device.vendorId)
                item.put("pid", device.productId)
                devices.put(item)
            }
            invoke.resolve(JSObject().put("devices", devices))
        } catch (error: Exception) {
            invoke.reject(error.message ?: "Failed to list USB devices")
        }
    }

    @Command
    fun openUsb(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(OpenUsbArgs::class.java)
            val device = usbManager.deviceList.values.firstOrNull { it.deviceName == args.path }
                ?: return invoke.reject("USB device not found: ${args.path}")
            if (!usbManager.hasPermission(device)) {
                pendingInvoke = invoke
                pendingDevice = device
                pendingBaud = args.baudRate
                val intent = PendingIntent.getBroadcast(
                    activity,
                    0,
                    Intent(ACTION_USB_PERMISSION),
                    PendingIntent.FLAG_IMMUTABLE
                )
                usbManager.requestPermission(device, intent)
                return
            }
            openUsbInternal(device, args.baudRate)
            invoke.resolve()
        } catch (error: Exception) {
            invoke.reject(error.message ?: "Failed to open USB serial device")
        }
    }

    private fun openUsbInternal(device: UsbDevice, baudRate: Int) {
        closeAll()
        closed = false
        val connection = usbManager.openDevice(device)
            ?: throw IllegalStateException("Android refused to open ${device.deviceName}")
        var selected: UsbInterface? = null
        var input: UsbEndpoint? = null
        var output: UsbEndpoint? = null

        for (index in 0 until device.interfaceCount) {
            val intf = device.getInterface(index)
            var candidateIn: UsbEndpoint? = null
            var candidateOut: UsbEndpoint? = null
            for (endpointIndex in 0 until intf.endpointCount) {
                val endpoint = intf.getEndpoint(endpointIndex)
                if (endpoint.type != UsbConstants.USB_ENDPOINT_XFER_BULK) continue
                if (endpoint.direction == UsbConstants.USB_DIR_IN) candidateIn = endpoint
                else candidateOut = endpoint
            }
            if (candidateIn != null && candidateOut != null) {
                selected = intf
                input = candidateIn
                output = candidateOut
                break
            }
        }

        if (selected == null || input == null || output == null) {
            connection.close()
            throw IllegalStateException("No CDC-ACM bulk endpoints on ${device.deviceName}")
        }

        if (!connection.claimInterface(selected, true)) {
            connection.close()
            throw IllegalStateException("Could not claim the USB serial interface")
        }

        val lineCoding = ByteArray(7)
        lineCoding[0] = (baudRate and 0xff).toByte()
        lineCoding[1] = ((baudRate shr 8) and 0xff).toByte()
        lineCoding[2] = ((baudRate shr 16) and 0xff).toByte()
        lineCoding[3] = ((baudRate shr 24) and 0xff).toByte()
        lineCoding[4] = 0
        lineCoding[5] = 0
        lineCoding[6] = 8
        connection.controlTransfer(0x21, 0x20, 0, selected.id, lineCoding, lineCoding.size, 0)
        connection.controlTransfer(0x21, 0x22, 3, selected.id, null, 0, 0)

        usbConnection = connection
        usbIn = input
        usbOut = output
        usbReader = Thread {
            val buffer = ByteArray(4096)
            var misses = 0
            while (!closed) {
                try {
                    val read = connection.bulkTransfer(input, buffer, buffer.size, 1000)
                    if (read > 0) {
                        misses = 0
                        emitData(buffer, read)
                    } else {
                        misses++
                        if (misses >= 3 &&
                            usbManager.deviceList.values.none { it.deviceName == device.deviceName }
                        ) {
                            break
                        }
                    }
                } catch (error: Exception) {
                    break
                }
            }
            if (!closed) emitDisconnect()
        }.also { it.start() }
    }

    private fun isLikelySpp(device: android.bluetooth.BluetoothDevice): Boolean {
        val major = device.bluetoothClass?.majorDeviceClass ?: return true
        return major != BluetoothClass.Device.Major.AUDIO_VIDEO &&
            major != BluetoothClass.Device.Major.PERIPHERAL &&
            major != BluetoothClass.Device.Major.IMAGING &&
            major != BluetoothClass.Device.Major.WEARABLE &&
            major != BluetoothClass.Device.Major.HEALTH
    }

    @Command
    fun listSpp(invoke: Invoke) {
        try {
            requireBluetoothPermission()
            val adapter = bluetoothAdapter()
            if (adapter == null || !adapter.isEnabled) {
                return invoke.reject("Bluetooth is disabled")
            }
            val devices = JSArray()
            for (device in adapter.bondedDevices) {
                if (!isLikelySpp(device)) continue
                val item = JSObject()
                item.put("address", device.address)
                item.put("name", device.name ?: device.address)
                devices.put(item)
            }
            invoke.resolve(JSObject().put("devices", devices))
        } catch (error: Exception) {
            invoke.reject(error.message ?: "Failed to list paired Bluetooth devices")
        }
    }

    @Command
    fun scanSpp(invoke: Invoke) {
        try {
            requireBluetoothPermission()
            val adapter = bluetoothAdapter() ?: return invoke.reject("Bluetooth is unavailable")
            if (!adapter.isEnabled) return invoke.reject("Bluetooth is disabled")
            val found = LinkedHashMap<String, JSObject>()

            fun addDevice(device: BluetoothDevice) {
                if (!isLikelySpp(device)) return
                val name = device.name
                if (name.isNullOrBlank()) return
                val item = JSObject()
                item.put("address", device.address)
                item.put("name", name)
                item.put("paired", device.bondState == BluetoothDevice.BOND_BONDED)
                found[device.address] = item
            }

            adapter.bondedDevices.forEach(::addDevice)
            val finished = AtomicBoolean(false)
            var receiver: BroadcastReceiver? = null

            fun finish() {
                if (!finished.compareAndSet(false, true)) return
                runCatching { adapter.cancelDiscovery() }
                receiver?.let { runCatching { activity.unregisterReceiver(it) } }
                val devices = JSArray()
                found.values.forEach { devices.put(it) }
                invoke.resolve(JSObject().put("devices", devices))
            }

            receiver = object : BroadcastReceiver() {
                override fun onReceive(context: Context, intent: Intent) {
                    when (intent.action) {
                        BluetoothDevice.ACTION_FOUND -> {
                            @Suppress("DEPRECATION")
                            val device = intent.getParcelableExtra<BluetoothDevice>(BluetoothDevice.EXTRA_DEVICE)
                            if (device != null) addDevice(device)
                        }
                        BluetoothAdapter.ACTION_DISCOVERY_FINISHED -> finish()
                    }
                }
            }
            val filter = IntentFilter().apply {
                addAction(BluetoothDevice.ACTION_FOUND)
                addAction(BluetoothAdapter.ACTION_DISCOVERY_FINISHED)
            }
            if (Build.VERSION.SDK_INT >= 33) {
                activity.registerReceiver(receiver, filter, Context.RECEIVER_EXPORTED)
            } else {
                @Suppress("DEPRECATION")
                activity.registerReceiver(receiver, filter)
            }
            if (!adapter.startDiscovery()) {
                finish()
            } else {
                Handler(Looper.getMainLooper()).postDelayed({ finish() }, 12000)
            }
        } catch (error: Exception) {
            invoke.reject(error.message ?: "Failed to scan for Bluetooth devices")
        }
    }

    private fun ensureBonded(device: BluetoothDevice, timeoutMs: Long = 30000): Boolean {
        if (device.bondState == BluetoothDevice.BOND_BONDED) return true
        val latch = CountDownLatch(1)
        var bonded = false
        val receiver = object : BroadcastReceiver() {
            override fun onReceive(context: Context, intent: Intent) {
                if (intent.action != BluetoothDevice.ACTION_BOND_STATE_CHANGED) return
                @Suppress("DEPRECATION")
                val changed = intent.getParcelableExtra<BluetoothDevice>(BluetoothDevice.EXTRA_DEVICE)
                if (changed?.address != device.address) return
                val state = intent.getIntExtra(BluetoothDevice.EXTRA_BOND_STATE, BluetoothDevice.BOND_NONE)
                if (state == BluetoothDevice.BOND_BONDED) {
                    bonded = true
                    latch.countDown()
                } else if (state != BluetoothDevice.BOND_BONDING) {
                    latch.countDown()
                }
            }
        }
        val filter = IntentFilter(BluetoothDevice.ACTION_BOND_STATE_CHANGED)
        try {
            if (Build.VERSION.SDK_INT >= 33) {
                activity.registerReceiver(receiver, filter, Context.RECEIVER_EXPORTED)
            } else {
                @Suppress("DEPRECATION")
                activity.registerReceiver(receiver, filter)
            }
            val started = device.bondState == BluetoothDevice.BOND_BONDING || device.createBond()
            if (!started) return false
            latch.await(timeoutMs, TimeUnit.MILLISECONDS)
            return bonded
        } catch (_: Exception) {
            return false
        } finally {
            runCatching { activity.unregisterReceiver(receiver) }
        }
    }

    @Command
    fun openSpp(invoke: Invoke) {
        try {
            requireBluetoothPermission()
            val args = invoke.parseArgs(OpenSppArgs::class.java)
            val adapter = bluetoothAdapter() ?: return invoke.reject("Bluetooth is unavailable")
            if (!adapter.isEnabled) return invoke.reject("Bluetooth is disabled")
            closeAll()
            closed = false
            val device = adapter.getRemoteDevice(args.address)
            adapter.cancelDiscovery()
            Thread {
                try {
                    if (device.bondState != BluetoothDevice.BOND_BONDED && !ensureBonded(device)) {
                        invoke.reject("The Bluetooth device is not paired. Confirm the system pairing prompt and try again.")
                        return@Thread
                    }
                    var lastError: Exception? = null
                    var connected: BluetoothSocket? = null
                    val factories: List<() -> BluetoothSocket> = listOf(
                        { device.createRfcommSocketToServiceRecord(SPP_UUID) },
                        { device.createInsecureRfcommSocketToServiceRecord(SPP_UUID) },
                        {
                            val method = device.javaClass.getMethod(
                                "createRfcommSocket",
                                Int::class.javaPrimitiveType,
                            )
                            method.invoke(device, 1) as BluetoothSocket
                        },
                    )
                    for (factory in factories) {
                        val candidate = try {
                            factory()
                        } catch (error: Exception) {
                            lastError = error
                            continue
                        }
                        var failure: Exception? = null
                        val latch = CountDownLatch(1)
                        Thread {
                            try {
                                candidate.connect()
                            } catch (error: Exception) {
                                failure = error
                            } finally {
                                latch.countDown()
                            }
                        }.start()
                        val finished = latch.await(6, TimeUnit.SECONDS)
                        if (finished && failure == null) {
                            connected = candidate
                            break
                        }
                        lastError = failure ?: Exception("Bluetooth SPP connection timed out")
                        Thread { runCatching { candidate.close() } }.start()
                    }

                    val socket = connected
                    if (socket == null) {
                        invoke.reject(lastError?.message ?: "Failed to connect to Bluetooth SPP")
                        return@Thread
                    }
                    sppSocket = socket
                    sppInput = socket.inputStream
                    sppOutput = socket.outputStream
                    invoke.resolve()
                    sppReader = Thread {
                        val buffer = ByteArray(4096)
                        while (!closed) {
                            try {
                                val read = socket.inputStream.read(buffer)
                                if (read <= 0) break
                                emitData(buffer, read)
                            } catch (error: Exception) {
                                break
                            }
                        }
                        if (!closed) emitDisconnect()
                    }.also { it.start() }
                } catch (error: Exception) {
                    invoke.reject(error.message ?: "Failed to connect to Bluetooth SPP")
                }
            }.start()
        } catch (error: Exception) {
            invoke.reject(error.message ?: "Failed to connect to Bluetooth SPP")
        }
    }

    @Command
    fun write(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(WriteArgs::class.java)
            val bytes = Base64.decode(args.data, Base64.DEFAULT)
            val connection = usbConnection
            val endpoint = usbOut
            if (connection != null && endpoint != null) {
                val written = connection.bulkTransfer(endpoint, bytes, bytes.size, 1000)
                if (written < 0) return invoke.reject("Failed to write to USB serial device")
                return invoke.resolve(JSObject().put("bytesWritten", written))
            }
            val output = sppOutput ?: return invoke.reject("No transport is open")
            output.write(bytes)
            output.flush()
            invoke.resolve(JSObject().put("bytesWritten", bytes.size))
        } catch (error: Exception) {
            invoke.reject(error.message ?: "Failed to write to transport")
        }
    }

    @Command
    fun close(invoke: Invoke) {
        closeAll()
        invoke.resolve()
    }

    private fun emitData(buffer: ByteArray, length: Int) {
        try {
            trigger("data", JSObject().put("data", Base64.encodeToString(buffer, 0, length, Base64.NO_WRAP)))
        } catch (error: Exception) {
            Log.w(TAG, "Cannot emit transport data", error)
        }
    }

    private fun emitDisconnect() {
        try {
            trigger("disconnect", JSObject())
        } catch (error: Exception) {
            Log.w(TAG, "Cannot emit transport disconnect", error)
        }
    }

    private fun bluetoothAdapter(): BluetoothAdapter? {
        val manager = activity.getSystemService(Context.BLUETOOTH_SERVICE) as? BluetoothManager
        return manager?.adapter
    }

    private fun requireBluetoothPermission() {
        if (Build.VERSION.SDK_INT >= 31 &&
            activity.checkSelfPermission(Manifest.permission.BLUETOOTH_CONNECT) != PackageManager.PERMISSION_GRANTED
        ) {
            throw IllegalStateException("Bluetooth permission is required. Grant Nearby devices access first.")
        }
    }

    private fun closeAll() {
        closed = true
        try {
            usbReader?.interrupt()
        } catch (_: Exception) {
        }
        try {
            usbConnection?.close()
        } catch (_: Exception) {
        }
        usbConnection = null
        usbIn = null
        usbOut = null
        usbReader = null

        try {
            sppReader?.interrupt()
        } catch (_: Exception) {
        }
        try {
            sppSocket?.close()
        } catch (_: Exception) {
        }
        sppSocket = null
        sppInput = null
        sppOutput = null
        sppReader = null
    }
}
