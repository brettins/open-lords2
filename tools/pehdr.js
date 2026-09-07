const fs=require('fs');const b=fs.readFileSync(process.argv[2]);
const pe=b.readUInt32LE(0x3c),opt=pe+24;
const ch=b.readUInt16LE(pe+22), dch=b.readUInt16LE(opt+70);
console.log('ImageBase      : 0x'+b.readUInt32LE(opt+28).toString(16));
console.log('EntryPoint RVA : 0x'+b.readUInt32LE(opt+16).toString(16),'-> VA 0x'+(b.readUInt32LE(opt+16)+b.readUInt32LE(opt+28)).toString(16));
console.log('Subsystem      : '+b.readUInt16LE(opt+68)+' (2=GUI)');
console.log('Characteristics: 0x'+ch.toString(16), (ch&1)?'[RELOCS_STRIPPED]':'[relocs present]');
console.log('DllCharacterist: 0x'+dch.toString(16), (dch&0x40)?'[DYNAMICBASE/ASLR]':'[no ASLR -> fixed base]', (dch&0x100)?'[NX]':'[no NX]');
console.log('SizeOfImage    : 0x'+b.readUInt32LE(opt+56).toString(16));
const nsec=b.readUInt16LE(pe+6),secOff=opt+b.readUInt16LE(pe+20);
for(let i=0;i<nsec;i++){const o=secOff+i*40;console.log('  sec '+b.toString('ascii',o,o+8).replace(/\0+$/,'').padEnd(8)+' VA=0x'+b.readUInt32LE(o+12).toString(16).padStart(6)+' vsize=0x'+b.readUInt32LE(o+8).toString(16).padStart(6)+' raw=0x'+b.readUInt32LE(o+20).toString(16).padStart(6)+' rsize=0x'+b.readUInt32LE(o+16).toString(16).padStart(6)+' flags=0x'+b.readUInt32LE(o+36).toString(16));}
