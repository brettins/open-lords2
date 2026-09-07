// Dump bytes: args = hexAddr length (repeatable pairs)
//@category Lords2
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;

public class DumpBytes extends GhidraScript {
    @Override public void run() throws Exception {
        String[] a = getScriptArgs();
        for (int i = 0; i + 1 < a.length; i += 2) {
            Address base = toAddr(Long.parseLong(a[i], 16));
            int len = Integer.parseInt(a[i+1]);
            byte[] b = new byte[len];
            currentProgram.getMemory().getBytes(base, b);
            StringBuilder hx = new StringBuilder(), as = new StringBuilder();
            for (int j = 0; j < len; j++) {
                hx.append(String.format("%02x ", b[j]));
                as.append(b[j] >= 32 && b[j] < 127 ? (char)b[j] : '.');
                if (j % 16 == 15) { println(base.add(j-15) + "  " + hx + " |" + as + "|"); hx.setLength(0); as.setLength(0); }
            }
            if (hx.length() > 0) println("  " + hx + " |" + as + "|");
        }
    }
}
