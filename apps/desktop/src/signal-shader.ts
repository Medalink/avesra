// Ported from approved design/mockups/Overlay.dc.html; signal generation is excluded.
export const fragmentShader = `
precision mediump float;
uniform sampler2D uBars;uniform float uN;uniform vec2 uRes;uniform float uDpr;uniform float uGlow;uniform float uMode;uniform float uTime;
uniform vec3 uCore;uniform vec3 uLow;uniform vec3 uHigh;
void main(){
  vec2 size=uRes/uDpr;
  vec2 fc=gl_FragCoord.xy/uDpr;
  float cy=size.y*0.5;
  float up=step(cy,fc.y);
  float dy=abs(fc.y-cy);
  if(uMode>0.5&&uMode<1.5){
    float x=fc.x/size.x;
    float i0=floor(fc.x);
    float s0=texture2D(uBars,vec2((i0+0.5)/uN,0.5)).r-0.5;
    float s1=texture2D(uBars,vec2((i0+1.5)/uN,0.5)).r-0.5;
    float tp=smoothstep(0.0,0.2,x)*smoothstep(1.0,0.8,x);
    float amp=(cy-4.0)*2.0*tp;
    float y0=cy+s0*amp;float y1=cy+s1*amp;
    float d=max(0.0,max(min(y0,y1)-0.7-fc.y,fc.y-max(y0,y1)-0.7));
    float tcore=smoothstep(1.2,0.0,d);
    float tglow=exp(-d*0.4)*0.75;
    float tcx=(fc.x-size.x*0.5)/(size.x*0.3);
    float tbloom=exp(-tcx*tcx)*exp(-dy/(cy*0.6))*0.18;
    float sx=floor(fc.x/6.0)*6.0;
    float qa=texture2D(uBars,vec2((sx+0.5)/uN,0.5)).r-0.5;
    float qb=texture2D(uBars,vec2((sx-5.5)/uN,0.5)).r-0.5;
    float tq=smoothstep(0.0,0.2,sx/size.x)*smoothstep(1.0,0.8,sx/size.x);
    float yq=cy+floor(qa*10.0+0.5)/10.0*(cy-4.0)*2.0*tq;
    float yp=cy+floor(qb*10.0+0.5)/10.0*(cy-4.0)*2.0*tq;
    float dq=abs(fc.y-yq);
    if(fc.x-sx<1.0){dq=min(dq,max(0.0,max(min(yq,yp)-fc.y,fc.y-max(yq,yp))));}
    float step1=smoothstep(1.0,0.0,dq)*0.55*tp;
    vec2 cell=floor(fc/vec2(4.0,4.0));
    float hsh=fract(sin(dot(cell,vec2(12.9898,78.233))+floor(uTime*9.0)*0.371)*43758.5453);
    float band=step(dy,(cy-4.0)*0.8*tp);
    vec2 inC=fract(fc/vec2(4.0,4.0));
    float bit=step(0.94,hsh)*band*step(inC.x,0.5)*step(inC.y,0.5)*0.7;
    vec3 tcol=uCore*tcore+uLow*(tglow+tbloom)+uHigh*step1+uCore*bit*0.8;
    float ta=clamp(tcore+tglow+tbloom+step1+bit*0.8,0.0,1.0);
    gl_FragColor=vec4(min(tcol,vec3(ta)),ta);
    return;
  }
  if(uMode>2.5){
    float x=fc.x/size.x;
    float pp=size.x/uN;
    float pos=fc.x/pp-0.5;
    float j0=floor(pos);
    float fr=pos-j0;
    float va=texture2D(uBars,vec2((j0+0.5)/uN,0.5)).r;
    float vb=texture2D(uBars,vec2((j0+1.5)/uN,0.5)).r;
    float env=mix(va,vb,fr*fr*(3.0-2.0*fr))*smoothstep(0.0,0.12,x)*smoothstep(1.0,0.88,x);
    float A=env*(cy-4.0);
    vec3 rc=vec3(0.0);float ra=0.0;
    for(int k=0;k<7;k++){
      float fk=float(k)/6.0;
      float tw=cos(fk*3.14159+uTime*0.9);
      float ph=x*18.0-uTime*3.2+fk*1.2;
      float y=cy+A*tw*sin(ph);
      float sl=A*tw*cos(ph)*18.0/size.x;
      float d=abs(fc.y-y)/sqrt(1.0+sl*sl);
      float ln=smoothstep(1.0,0.0,d)*(0.4+0.6*abs(tw));
      float g2=exp(-d*0.7)*0.16;
      rc+=mix(uLow,uHigh,fk)*(ln+g2);ra+=ln+g2;
    }
    float rcore=smoothstep(1.2,0.0,dy)*0.45;
    float rcx=(fc.x-size.x*0.5)/(size.x*0.3);
    float rbloom=exp(-rcx*rcx)*exp(-dy/(cy*0.6))*0.2;
    rc+=uCore*rcore+uLow*rbloom;ra+=rcore+rbloom;
    ra=clamp(ra,0.0,1.0);
    gl_FragColor=vec4(min(rc,vec3(ra)),ra);
    return;
  }
  if(uMode>1.5){
    float pp=size.x/uN;
    float pos=fc.x/pp-0.5;
    float j0=floor(pos);
    float fr=pos-j0;
    float va=texture2D(uBars,vec2((j0+0.5)/uN,0.5)).r;
    float vb=texture2D(uBars,vec2((j0+1.5)/uN,0.5)).r;
    float v=mix(va,vb,fr*fr*(3.0-2.0*fr));
    float ex=fc.x/size.x;
    float et=smoothstep(0.0,0.08,ex)*smoothstep(1.0,0.92,ex);
    float eh=max(0.6,v*(cy-3.0)*et);
    float inE=step(dy,eh);
    float rr=clamp(dy/eh,0.0,1.0);
    float stripe=0.55+0.45*step(1.0,mod(fc.x,3.0));
    float fill=inE*(0.12+0.38*rr*rr)*stripe*mix(0.8,1.0,up);
    float edge=smoothstep(1.2,0.0,abs(dy-eh))*0.95;
    float ecore=smoothstep(1.3,0.0,dy)*uGlow;
    float eglow=exp(-dy*0.5)*0.35*uGlow;
    float ecx=(fc.x-size.x*0.5)/(size.x*0.3);
    float ebloom=exp(-ecx*ecx)*exp(-dy/(cy*0.6))*0.16;
    vec3 ecol=mix(uLow,uHigh,rr)*fill+uLow*edge+uCore*ecore+uLow*(eglow+ebloom);
    float ea=clamp(fill+edge+ecore+eglow+ebloom,0.0,1.0);
    gl_FragColor=vec4(min(ecol,vec3(ea)),ea);
    return;
  }
  float pitch=size.x/uN;
  float idx=floor(fc.x/pitch);
  float lx=fc.x-idx*pitch;
  float barW=max(2.0,pitch*0.52);
  float inBar=step(abs(lx-pitch*0.5),barW*0.5);
  float amp=texture2D(uBars,vec2((idx+0.5)/uN,0.5)).r;
  float hh=amp*(cy-2.0)*mix(0.86,1.0,up);
  float inside=step(dy,hh)*step(1.5,dy);
  float r=clamp(dy/max(hh,0.001),0.0,1.0);
  float fade=(1.0-r*0.55)*mix(0.75,1.0,up);
  float bar=inBar*inside*fade;
  vec3 bc=mix(uLow,uHigh,r);
  float core=smoothstep(1.3,0.0,dy)*uGlow;
  float glow=exp(-dy*0.55)*0.6*uGlow;
  float cx=(fc.x-size.x*0.5)/(size.x*0.28);
  float bloom=exp(-cx*cx)*exp(-dy/(cy*0.55))*0.22*uGlow;
  vec3 col=bc*bar+uCore*core+uLow*(glow*0.8+bloom);
  float a=clamp(bar+core+glow*0.8+bloom,0.0,1.0);
  gl_FragColor=vec4(min(col,vec3(a)),a);
}
`;
