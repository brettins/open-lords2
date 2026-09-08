// Dump bytes / disassembly to a file. args: outFile mode ...
//   "bytes" hexAddr len [hexAddr len ...]
//   "disasm" hexAddr count
//@category Lords2
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Instruction;
import java.io.*;
public class KDump extends GhidraScript {
    @Override public void run() throws Exception {
        String[] a = getScriptArgs();
        PrintWriter out = new PrintWriter(new BufferedWriter(new FileWriter(a[0])));
        String mode = a[1];
        if (mode.equals("bytes")) {
            for (int i = 2; i + 1 < a.length; i += 2) {
                Address base = toAddr(Long.parseLong(a[i],16));
                int len = Integer.parseInt(a[i+1]);
                byte[] b = new byte[len];
                currentProgram.getMemory().getBytes(base, b);
                StringBuilder hx=new StringBuilder(), as=new StringBuilder();
                for (int j=0;j<len;j++){
                    hx.append(String.format("%02x ", b[j]));
                    as.append(b[j]>=32&&b[j]<127?(char)b[j]:'.');
                    if(j%16==15){out.println(base.add(j-15)+"  "+hx+" |"+as+"|");hx.setLength(0);as.setLength(0);}
                }
                if(hx.length()>0) out.println("  "+hx+" |"+as+"|");
            }
        } else if (mode.equals("disasm")) {
            Address addr = toAddr(a[2]);
            int n = Integer.parseInt(a[3]);
            Instruction ins = getInstructionAt(addr);
            for (int i=0;i<n&&ins!=null;i++){ out.println(ins.getAddress()+"  "+ins); ins = ins.getNext(); }
        }
        out.close();
        println("### wrote " + a[0]);
    }
}
