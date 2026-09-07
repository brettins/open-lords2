const {tail,sheet}=require('E:/dev/lords2/tools/maps/lib.js');
const t=tail(+(process.argv[2]||0));
sheet([{data:t,w:65,h:129,mode:'idx'},{data:t,w:129,h:65,mode:'idx'},{data:t.slice(0,4225),w:65,h:65,mode:'idx'},{data:t.slice(4225),w:65,h:64,mode:'idx'}],
      129,129,4,4,'E:/dev/lords2/tools/maps/out/tail_views.png',10);
