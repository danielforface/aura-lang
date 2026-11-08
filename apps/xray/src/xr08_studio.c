/*
 * AURA X-RAY XR-08 — first truly native Windows investigation window.
 * NO HTML, Electron, browser, JavaScript, network, Raylib or runtime installs.
 * Canvas = Windows GDI only; it DOES NOT decide layout, hit testing, policy,
 * navigation, score visualization, classification or evidence source mapping.
 * Those decisions are always made by genuine code generated from studio.aura
 * and analysis.aura. This is NOT yet a pure Aura Win32 FFI or Lumina backend.
 */
#define WIN32_LEAN_AND_MEAN
#define _CRT_SECURE_NO_WARNINGS
#include <windows.h>
#include <windowsx.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <limits.h>
#include "xr08_studio.h"

/* Native ABI: each symbol must exist in the Aura-generated studio/module.c. */
extern uint32_t xray08_ui_window_min_w(void);
extern uint32_t xray08_ui_window_min_h(void);
extern uint32_t xray08_ui_tab_y(void);
extern uint32_t xray08_ui_tab_x(uint32_t);
extern uint32_t xray08_ui_tab_hit(uint32_t,uint32_t);
extern uint32_t xray08_ui_cycle_policy(uint32_t,uint32_t);
extern uint32_t xray08_ui_case_x(void);
extern uint32_t xray08_ui_case_y(void);
extern uint32_t xray08_ui_case_w(void);
extern uint32_t xray08_ui_case_step(void);
extern uint32_t xray08_ui_case_row_y(uint32_t);
extern uint32_t xray08_ui_case_hit(uint32_t,uint32_t);
extern uint32_t xray08_ui_case_index(uint32_t,uint32_t,uint32_t);
extern uint32_t xray08_ui_case_page_for(uint32_t);
extern uint32_t xray08_ui_cycle_case(uint32_t,uint32_t,uint32_t);
extern uint32_t xray08_ui_page_next(uint32_t,uint32_t);
extern uint32_t xray08_ui_page_prev(uint32_t);
extern uint32_t xray08_ui_chart_x(void);
extern uint32_t xray08_ui_chart_width(uint32_t);
extern uint32_t xray08_ui_chart_sample_x(uint32_t,uint32_t,uint32_t);
extern uint32_t xray08_ui_chart_height(uint32_t,uint32_t);
extern uint32_t xray08_ui_chart_hit(uint32_t,uint32_t,uint32_t,uint32_t);
extern uint32_t xray08_ui_detail_x(uint32_t);
extern uint32_t xray08_ui_detail_w(void);
extern uint32_t xray08_ui_evidence_tone(uint32_t,uint32_t);
extern uint32_t xray08_ui_score_tone(uint32_t);
extern uint32_t xray07_policy_support(uint32_t,uint32_t,uint32_t);

typedef struct {
    XR08StudioModel *data; /* strictly validated, immutable once published */
    uint32_t policy,selected,page,focus_sample;
} XR08UI;

static const COLORREF C_BG=RGB(7,13,23),C_CARD=RGB(15,26,40);
static const COLORREF C_SUB=RGB(19,34,52),C_TEXT=RGB(226,238,247);
static const COLORREF C_DIM=RGB(123,150,172),C_LINE=RGB(35,56,75);
static const COLORREF C_AURA=RGB(75,236,211),C_BLUE=RGB(92,156,241);
static const COLORREF C_WARM=RGB(255,189,108),C_RED=RGB(246,124,141);

static void rect_fill(HDC dc,int x,int y,int w,int h,COLORREF color){
    if(w<=0||h<=0)return;
    RECT r={x,y,x+w,y+h};
    HBRUSH b=CreateSolidBrush(color);
    if(b){FillRect(dc,&r,b);DeleteObject(b);}
}
static void round_fill(HDC dc,int x,int y,int w,int h,int rad,COLORREF color){
    if(w<=0||h<=0)return;
    HBRUSH b=CreateSolidBrush(color);
    if(!b)return;
    HGDIOBJ oldbrush=SelectObject(dc,b);
    HGDIOBJ oldpen=SelectObject(dc,GetStockObject(NULL_PEN));
    RoundRect(dc,x,y,x+w,y+h,rad,rad);
    SelectObject(dc,oldpen);SelectObject(dc,oldbrush);
    DeleteObject(b);
}
static void rule(HDC dc,int x,int y,int x2,int y2,COLORREF color){
    HPEN p=CreatePen(PS_SOLID,1,color);
    if(!p)return;
    HGDIOBJ prior=SelectObject(dc,p);
    MoveToEx(dc,x,y,NULL);LineTo(dc,x2,y2);
    SelectObject(dc,prior);DeleteObject(p);
}
static HFONT font(int px,int weight){
    return CreateFontA(-px,0,0,0,weight,0,0,0,DEFAULT_CHARSET,
                       OUT_DEFAULT_PRECIS,CLIP_DEFAULT_PRECIS,
                       CLEARTYPE_QUALITY,DEFAULT_PITCH|FF_SWISS,"Segoe UI");
}
static void label(HDC dc,HFONT f,int x,int y,COLORREF c,const char *s){
    if(!s)return;
    HGDIOBJ old=f?SelectObject(dc,f):NULL;
    SetBkMode(dc,TRANSPARENT);SetTextColor(dc,c);
    TextOutA(dc,x,y,s,(int)strlen(s));
    if(f)SelectObject(dc,old);
}
static const char *kind_label(uint32_t k){
    switch(k){
        case 1:return "MEMORY PULSE";
        case 2:return "SUSTAINED CPU";
        case 3:return "I/O TRANSFER";
        case 4:return "CONCURRENT SIGNALS";
        default:return "UNKNOWN";
    }
}
static const char *phase_label(uint8_t p){
    switch(p){
        case 1:return "PENDING / NOT YET DECIDED";
        case 2:return "OBSERVED RETRACTION";
        case 3:return "WINDOW ELAPSED";
        case 4:return "EVIDENCE INTERRUPTED";
        default:return "NOT APPLICABLE";
    }
}
static const char *profile_label(uint32_t p){
    return p==0?"SENSITIVE":p==1?"STANDARD":"STRICT";
}
static uint32_t source_support(const XR08StudioModel *m,const XR05Case *c){
    uint32_t flags[3]={0,0,0};
    for(unsigned p=0;p<3;p++){
        for(unsigned i=0;i<m->policy[p].count;i++){
            const XR05Case *r=&m->policy[p].items[i];
            if(r->kind==c->kind && r->pid==c->pid &&
               r->creation==c->creation &&
               r->start<=c->end && c->start<=r->end){
                flags[p]=1;break;
            }
        }
    }
    /* The policy-support aggregation is another compiled Aura function. */
    return xray07_policy_support(flags[0],flags[1],flags[2]);
}
static uint32_t bound_chart_score(uint32_t value){
    /* Only the PLOT scale is bounded. Text/JSON retain true source score. */
    return value>1000000u?1000000u:value;
}
static void paint_studio(HDC dc,unsigned vieww,unsigned viewh,const XR08UI *ui){
    const XR08StudioModel *m=ui->data;
    if(!m)return;
    if(vieww<xray08_ui_window_min_w())vieww=xray08_ui_window_min_w();
    if(viewh<xray08_ui_window_min_h())viewh=xray08_ui_window_min_h();
    if(vieww>4096)vieww=4096;
    if(viewh>4096)viewh=4096;
    const uint32_t p=ui->policy<=2?ui->policy:1;
    const XR05Casebook *book=&m->policy[p];
    const int left=(int)xray08_ui_case_x();
    const int listw=(int)xray08_ui_case_w();
    const int chartx=(int)xray08_ui_chart_x();
    const int chartw=(int)xray08_ui_chart_width(vieww);
    const int sidex=(int)xray08_ui_detail_x(vieww);
    const int sidew=(int)xray08_ui_detail_w();
    const int basey=608;
    const uint32_t n=m->actual_samples;
    HFONT fBig=font(49,FW_BOLD),fTitle=font(23,FW_BOLD);
    HFONT fSemi=font(17,FW_SEMIBOLD),fBody=font(16,FW_NORMAL);
    HFONT fSmall=font(13,FW_NORMAL);
    rect_fill(dc,0,0,(int)vieww,(int)viewh,C_BG);
    rect_fill(dc,0,0,(int)vieww,5,C_AURA);
    label(dc,fSmall,32,29,C_AURA,"AURA / NATIVE OBSERVABILITY");
    label(dc,fBig,29,51,C_TEXT,"X-RAY");
    label(dc,fSemi,250,81,C_DIM,"STUDIO  /  XR-08  /  COMPILED AURA UI");
    label(dc,fSmall,sidex,39,C_DIM,"LOCAL-ONLY   |   NO BROWSER");
    label(dc,fSmall,sidex,58,C_DIM,"READ-ONLY   |   NO PRIVILEGES");
    rule(dc,30,109,(int)vieww-28,109,C_LINE);
    for(unsigned i=0;i<3;i++){
        const int x=(int)xray08_ui_tab_x(i);
        const int y=(int)xray08_ui_tab_y();
        round_fill(dc,x,y,145,42,14,i==p?RGB(29,88,90):C_CARD);
        if(i==p)rect_fill(dc,x+12,y+38,121,3,C_AURA);
        label(dc,fSemi,x+15,y+10,i==p?C_AURA:C_DIM,profile_label(i));
        char number[24];
        (void)snprintf(number,sizeof number,"%u",m->policy[i].count);
        label(dc,fSemi,x+113,y+10,i==p?C_TEXT:C_DIM,number);
    }
    label(dc,fSmall,sidex,140,C_DIM,"<- -> POLICY    |    UP/DOWN CASE    |    PGUP/DN PAGE");

    round_fill(dc,left,198,listw,viewh-225,20,C_CARD);
    label(dc,fTitle,left+19,216,C_TEXT,"EVIDENCE CASES");
    char buf[224];
    (void)snprintf(buf,sizeof buf,"PAGE %u / %u  |  %u verified pattern(s)",
                   (unsigned)ui->page+1,
                   book->count?(book->count+7u)/8u:1u,book->count);
    label(dc,fSmall,left+20,244,C_DIM,buf);
    for(unsigned row=0;row<8;row++){
        const uint32_t selected=xray08_ui_case_index(ui->page,row+1,book->count);
        if(selected==0)break;
        const uint32_t index=selected-1;
        const XR05Case *c=&book->items[index];
        const int y=(int)xray08_ui_case_row_y(row)+13;
        const int isActive=(index==ui->selected);
        round_fill(dc,left+13,y,listw-26,51,10,
                   isActive?RGB(28,70,83):C_SUB);
        if(isActive)rect_fill(dc,left+13,y+6,3,39,C_AURA);
        (void)snprintf(buf,sizeof buf,"PID %-6u   %s",c->pid,kind_label(c->kind));
        label(dc,fSemi,left+27,y+6,isActive?C_TEXT:C_DIM,buf);
        const uint32_t support=source_support(m,c);
        (void)snprintf(buf,sizeof buf,"S%u-S%u    grade %u   |   %u/3 policies",
                       c->start,c->end,(unsigned)c->grade,support);
        label(dc,fSmall,left+27,y+29,isActive?C_AURA:C_DIM,buf);
    }
    if(!book->count){
        label(dc,fSemi,left+20,307,C_DIM,"NO PATTERNS UNDER THIS POLICY");
        label(dc,fSmall,left+20,338,C_DIM,"This is not proof that the host is healthy.");
    }
    label(dc,fSmall,left+18,(int)viewh-56,C_DIM,
          "CLICK A CASE. RULES/STATE ARE AURA-NATIVE.");

    round_fill(dc,chartx-16,198,chartw+37,462,20,C_CARD);
    label(dc,fTitle,chartx,218,C_TEXT,"ACTIVITY / TIME");
    label(dc,fSmall,chartx,248,C_DIM,
          "Peak Aura activity score per sample  |  observed, not predicted");
    uint32_t maxscore=1;
    for(uint32_t i=1;i<=n;i++){
        const uint32_t v=bound_chart_score(m->sample_peak[i]);
        if(v>maxscore)maxscore=v;
    }
    for(unsigned k=0;k<4;k++){
        const int y=basey-((int)k*63);
        rule(dc,chartx,y,chartx+chartw-28,y,C_LINE);
    }
    for(uint32_t i=1;i<=n;i++){
        const int x=(int)xray08_ui_chart_sample_x(i,n,vieww);
        const uint32_t val=bound_chart_score(m->sample_peak[i]);
        int ht=(int)xray08_ui_chart_height(val,maxscore);
        if(val && ht<2)ht=2;
        const COLORREF bar=ui->focus_sample==i?C_WARM:
                  xray08_ui_score_tone(val)>1?C_AURA:C_BLUE;
        round_fill(dc,x-9,basey-ht,18,ht,7,bar);
        (void)snprintf(buf,sizeof buf,"%u",i);
        label(dc,fSmall,x-5,basey+9,C_DIM,buf);
        if(ui->focus_sample==i){
            (void)snprintf(buf,sizeof buf,"%u",m->sample_peak[i]);
            label(dc,fSmall,x-5,basey-ht-22,C_WARM,buf);
        }
    }
    (void)snprintf(buf,sizeof buf,"SAMPLE %u   |   PEAK SCORE %u   |   OBSERVED PROCESSES %u",
                   ui->focus_sample,
                   ui->focus_sample<=n?m->sample_peak[ui->focus_sample]:0u,
                   ui->focus_sample<=n?m->sample_count[ui->focus_sample]:0u);
    label(dc,fSmall,chartx,678,C_DIM,buf);
    (void)snprintf(buf,sizeof buf,"SOURCE: %llu observations  /  %llu comparable  /  %llu raw-verified",
                   (unsigned long long)m->records,
                   (unsigned long long)m->derived,
                   (unsigned long long)m->raw_verified);
    label(dc,fSmall,chartx,709,C_AURA,buf);
    label(dc,fSmall,chartx,732,C_DIM,
          "Chart geometry, navigation and hit testing are compiled from studio.aura.");

    round_fill(dc,sidex-12,198,sidew+14,viewh-225,20,C_CARD);
    label(dc,fSmall,sidex,218,C_AURA,"SOURCE-CONNECTED INVESTIGATION");
    if(book->count && ui->selected<book->count){
        const XR05Case *c=&book->items[ui->selected];
        label(dc,fTitle,sidex,252,C_TEXT,kind_label(c->kind));
        (void)snprintf(buf,sizeof buf,"PID %u   /   S%u - S%u",c->pid,c->start,c->end);
        label(dc,fSemi,sidex,290,C_AURA,buf);
        (void)snprintf(buf,sizeof buf,"MAXIMUM   %u",c->peak_value);
        label(dc,fSemi,sidex,327,C_TEXT,buf);
        label(dc,fSmall,sidex,354,C_DIM,
              c->kind==1?"Peak measured memory growth (KiB)":
              c->kind==2?"Peak measured CPU (millicores)":
              c->kind==3?"Peak measured transfer (KiB/s)":"Coincident dimensions");
        if(c->kind==1){
            label(dc,fSmall,sidex,380,C_DIM,phase_label(c->phase));
            (void)snprintf(buf,sizeof buf,"Observed retraction  %u KiB",c->recovered_kib);
            label(dc,fSmall,sidex,404,C_TEXT,buf);
        }else{
            label(dc,fSmall,sidex,380,C_DIM,phase_label(c->phase));
        }
        const uint32_t support=source_support(m,c);
        const uint32_t tone=xray08_ui_evidence_tone(c->grade,support);
        (void)snprintf(buf,sizeof buf,"%u/3 PROFILES    /    EVIDENCE GRADE %u",
                       support,(unsigned)c->grade);
        round_fill(dc,sidex,432,sidew-10,33,8,tone>=15?RGB(29,78,73):C_SUB);
        label(dc,fSmall,sidex+11,441,tone>=15?C_AURA:C_DIM,buf);
        rule(dc,sidex,478,sidex+sidew-17,478,C_LINE);
        label(dc,fSmall,sidex,489,C_DIM,"ACTUAL SOURCE SAMPLES  (SAME PID + START TIME)");
        unsigned matching=0,shown=0;
        for(size_t k=0;k<m->event_count;k++){
            const XR05Observation *o=&m->events[k];
            if(o->pid!=c->pid||o->creation!=c->creation ||
               (c->start>1 && o->sample<c->start-1)||
               o->sample>c->end+1)continue;
            matching++;
            if(shown>=9)continue;
            char val[220];
            if(o->comparable){
                (void)snprintf(val,sizeof val,"S%-2u   CPU %-5u  I/O %-6u  WS %+lld",
                               o->sample,o->cpu,
                               o->read>o->write?o->read:o->write,
                               (long long)o->delta_kib);
            }else{
                (void)snprintf(val,sizeof val,"S%-2u   UNKNOWN / BASELINE",o->sample);
            }
            label(dc,fSmall,sidex,519+(int)(shown*25),
                  o->rate_reverified?C_TEXT:C_DIM,val);
            shown++;
        }
        if(matching>shown){
            (void)snprintf(buf,sizeof buf,"%u / %u source rows visible; full data in capture.",
                           shown,matching);
            label(dc,fSmall,sidex,747,C_WARM,buf);
        }
    }else{
        label(dc,fSemi,sidex,272,C_DIM,"NO SELECTED CASE");
        label(dc,fSmall,sidex,310,C_DIM,
              "Try another Aura policy or inspect source samples.");
    }
    rule(dc,30,(int)viewh-33,(int)vieww-28,(int)viewh-33,C_LINE);
    (void)snprintf(buf,sizeof buf,
           "AURA X-RAY / NATIVE WIN32 STUDIO     |     CAPTURE BUILD %.12s     |     POLICY: %s",
           m->build,profile_label(p));
    label(dc,fSmall,34,(int)viewh-26,C_DIM,buf);
    if(fBig)DeleteObject(fBig);
    if(fTitle)DeleteObject(fTitle);
    if(fSemi)DeleteObject(fSemi);
    if(fBody)DeleteObject(fBody);
    if(fSmall)DeleteObject(fSmall);
}
static void invalidate(HWND hwnd){InvalidateRect(hwnd,NULL,FALSE);}
static void select_profile(XR08UI *u,uint32_t p){
    if(p>2)return;
    u->policy=p;u->selected=0;u->page=0;u->focus_sample=0;
}
static LRESULT CALLBACK window_proc(HWND hwnd,UINT msg,WPARAM wp,LPARAM lp){
    XR08UI *u=(XR08UI*)GetWindowLongPtrA(hwnd,GWLP_USERDATA);
    switch(msg){
        case WM_NCCREATE: {
            const CREATESTRUCTA *c=(const CREATESTRUCTA*)lp;
            SetWindowLongPtrA(hwnd,GWLP_USERDATA,(LONG_PTR)c->lpCreateParams);
            return TRUE;
        }
        case WM_GETMINMAXINFO: {
            MINMAXINFO *mmi=(MINMAXINFO*)lp;
            mmi->ptMinTrackSize.x=(LONG)xray08_ui_window_min_w()+20;
            mmi->ptMinTrackSize.y=(LONG)xray08_ui_window_min_h()+60;
            return 0;
        }
        case WM_ERASEBKGND:return 1;
        case WM_PAINT:{
            PAINTSTRUCT ps;HDC dc=BeginPaint(hwnd,&ps);
            if(dc && u){
                RECT r;GetClientRect(hwnd,&r);
                paint_studio(dc,(unsigned)r.right,(unsigned)r.bottom,u);
            }
            EndPaint(hwnd,&ps);
            return 0;
        }
        case WM_LBUTTONDOWN:{
            if(!u||!u->data)return 0;
            int ix=GET_X_LPARAM(lp),iy=GET_Y_LPARAM(lp);
            if(ix<0||iy<0)return 0;
            const uint32_t x=(uint32_t)ix,y=(uint32_t)iy;
            RECT r;GetClientRect(hwnd,&r);
            unsigned w=(unsigned)r.right;
            if(w<1100u)w=1100u;
            if(w>4096u)w=4096u;
            uint32_t tab=xray08_ui_tab_hit(x,y);
            if(tab>=1&&tab<=3)select_profile(u,tab-1);
            else{
                const XR05Casebook *cases=&u->data->policy[u->policy];
                uint32_t row=xray08_ui_case_hit(x,y);
                if(row){
                    uint32_t selected=xray08_ui_case_index(u->page,row,cases->count);
                    if(selected)u->selected=selected-1;
                }else{
                    uint32_t sample=xray08_ui_chart_hit(x,y,w,u->data->actual_samples);
                    if(sample>=1&&sample<=u->data->actual_samples)
                        u->focus_sample=sample;
                }
            }
            SetFocus(hwnd);invalidate(hwnd);return 0;
        }
        case WM_KEYDOWN:{
            if(!u||!u->data)return 0;
            const XR05Casebook *b=&u->data->policy[u->policy];
            if(wp==VK_RIGHT||wp==VK_LEFT)
                select_profile(u,xray08_ui_cycle_policy(
                    u->policy,wp==VK_LEFT?1u:0u));
            else if((wp==VK_DOWN||wp==VK_UP) && b->count){
                u->selected=xray08_ui_cycle_case(
                    u->selected,b->count,wp==VK_UP?1u:0u);
                u->page=xray08_ui_case_page_for(u->selected);
            }else if(wp==VK_NEXT && b->count){
                u->page=xray08_ui_page_next(u->page,b->count);
                const uint32_t selected=xray08_ui_case_index(u->page,1,b->count);
                if(selected)u->selected=selected-1;
            }else if(wp==VK_PRIOR){
                u->page=xray08_ui_page_prev(u->page);
                const uint32_t selected=xray08_ui_case_index(u->page,1,b->count);
                if(selected)u->selected=selected-1;
            }else if(wp==VK_ESCAPE){
                DestroyWindow(hwnd);return 0;
            }
            invalidate(hwnd);return 0;
        }
        case WM_DESTROY:PostQuitMessage(0);return 0;
        default:return DefWindowProcA(hwnd,msg,wp,lp);
    }
}
static void errtxt(char *out,size_t cap,const char *msg){
    if(out&&cap)(void)snprintf(out,cap,"%s",msg);
}
static int save_preview(const char *path,const XR08UI *ui,char *error,size_t cap){
    const int width=(int)XR08_VIEW_W,height=(int)XR08_VIEW_H;
    BITMAPINFO bmi={0};
    bmi.bmiHeader.biSize=sizeof(BITMAPINFOHEADER);
    bmi.bmiHeader.biWidth=width;
    bmi.bmiHeader.biHeight=-height; /* top-down real GDI frame */
    bmi.bmiHeader.biPlanes=1;
    bmi.bmiHeader.biBitCount=32;
    bmi.bmiHeader.biCompression=BI_RGB;
    bmi.bmiHeader.biSizeImage=(DWORD)(width*height*4);
    void *bits=NULL;
    HDC dc=CreateCompatibleDC(NULL);
    HBITMAP bmp=CreateDIBSection(dc,&bmi,DIB_RGB_COLORS,&bits,NULL,0);
    if(!dc||!bmp||!bits){
        if(bmp)DeleteObject(bmp);
        if(dc)DeleteDC(dc);
        errtxt(error,cap,"native 32-bit GDI surface unavailable");
        return 0;
    }
    HGDIOBJ old=SelectObject(dc,bmp);
    paint_studio(dc,width,height,ui);
    GdiFlush();
    FILE *f=NULL;
    if(fopen_s(&f,path,"wbx")!=0||!f){
        SelectObject(dc,old);DeleteObject(bmp);DeleteDC(dc);
        errtxt(error,cap,"preview target exists or cannot be created");
        return 0;
    }
    BITMAPFILEHEADER fh={0};
    fh.bfType=0x4D42;
    fh.bfOffBits=(DWORD)(sizeof fh+sizeof(BITMAPINFOHEADER));
    fh.bfSize=fh.bfOffBits+bmi.bmiHeader.biSizeImage;
    int ok=fwrite(&fh,sizeof fh,1,f)==1 &&
           fwrite(&bmi.bmiHeader,sizeof bmi.bmiHeader,1,f)==1 &&
           fwrite(bits,1,bmi.bmiHeader.biSizeImage,f)==bmi.bmiHeader.biSizeImage;
    if(fclose(f)!=0)ok=0;
    SelectObject(dc,old);DeleteObject(bmp);DeleteDC(dc);
    if(!ok){remove(path);errtxt(error,cap,"failed to write complete bitmap");}
    return ok;
}
int xr08_studio_self_test(void){
    if(xray08_ui_window_min_w()!=1100||xray08_ui_window_min_h()!=760||
       xray08_ui_tab_y()!=122||
       xray08_ui_tab_x(0)!=36||xray08_ui_tab_x(2)!=350||
       xray08_ui_tab_hit(60,130)!=1||
       xray08_ui_tab_hit(220,130)!=2||
       xray08_ui_tab_hit(375,130)!=3||
       xray08_ui_tab_hit(10,130)!=0||
       xray08_ui_tab_hit(80,185)!=0||
       xray08_ui_cycle_policy(0,1)!=2||
       xray08_ui_cycle_policy(2,0)!=0||
       xray08_ui_case_x()!=30||xray08_ui_case_y()!=252||
       xray08_ui_case_w()!=390||xray08_ui_case_step()!=56||
       xray08_ui_case_row_y(3)!=420||
       xray08_ui_case_hit(55,263)!=1||
       xray08_ui_case_hit(55,650)!=8||
       xray08_ui_case_hit(450,300)!=0||
       xray08_ui_case_index(1,1,11)!=9||
       xray08_ui_case_index(1,4,11)!=0||
       xray08_ui_case_page_for(9)!=1||
       xray08_ui_cycle_case(0,3,1)!=2||
       xray08_ui_cycle_case(2,3,0)!=0||
       xray08_ui_cycle_case(0,1,1)!=0||
       xray08_ui_page_next(0,17)!=1||
       xray08_ui_page_next(2,17)!=2||
       xray08_ui_page_prev(0)!=0||
       xray08_ui_page_prev(2)!=1||
       xray08_ui_chart_x()!=455||
       xray08_ui_chart_width(1360)!=480||
       xray08_ui_chart_sample_x(1,10,1360)!=498||
       xray08_ui_chart_height(50,99)!=98||
       xray08_ui_chart_height(0,0)!=0||
       xray08_ui_chart_hit(501,480,1360,10)!=1||
       xray08_ui_chart_hit(100,480,1360,10)!=0||
       xray08_ui_detail_x(1360)!=1000||
       xray08_ui_detail_w()!=328||
       xray08_ui_evidence_tone(3,3)!=15||
       xray08_ui_score_tone(0)!=0||
       xray08_ui_score_tone(12)!=1||
       xray08_ui_score_tone(50)!=2){
        fputs("XR08 AURA UI CONTRACT FAIL: pixel, state or keyboard semantics\n",stderr);
        return 0;
    }
    puts("AURA_XR08_STUDIO_PASS 36 native Aura UI vectors; semantics=compiled");
    return 1;
}
int xr08_studio(const char *path,const char *preview_bmp,
                char *error,size_t cap){
    if(!path||!path[0]){errtxt(error,cap,"missing source capture");return 0;}
    XR08StudioModel *m=(XR08StudioModel*)calloc(1,sizeof(*m));
    XR08UI *ui=(XR08UI*)calloc(1,sizeof(*ui));
    if(!m||!ui){
        free(m);free(ui);errtxt(error,cap,"native studio allocation failed");return 0;
    }
    int ok=xr03_load_studio(path,m,error,cap);
    if(!ok){free(m->events);free(m);free(ui);return 0;}
    ui->data=m;ui->policy=1;ui->selected=0;ui->page=0;
    ui->focus_sample=m->actual_samples;
    if(preview_bmp){
        ok=save_preview(preview_bmp,ui,error,cap);
    }else{
        HINSTANCE inst=GetModuleHandleA(NULL);
        WNDCLASSEXA cls={0};cls.cbSize=sizeof(cls);
        cls.hInstance=inst;cls.lpfnWndProc=window_proc;
        cls.hCursor=LoadCursorA(NULL,IDC_ARROW);
        cls.lpszClassName="AURA_XRAY_NATIVE_STUDIO_08";
        cls.hbrBackground=(HBRUSH)GetStockObject(BLACK_BRUSH);
        if(!RegisterClassExA(&cls) && GetLastError()!=ERROR_CLASS_ALREADY_EXISTS){
            errtxt(error,cap,"registering native Windows studio failed");ok=0;
        }else{
            HWND hwnd=CreateWindowExA(0,cls.lpszClassName,
                "AURA X-RAY / Native Studio / Compiled Aura UI",
                WS_OVERLAPPEDWINDOW|WS_VISIBLE,
                CW_USEDEFAULT,CW_USEDEFAULT,1410,920,
                NULL,NULL,inst,ui);
            if(!hwnd){
                errtxt(error,cap,"native Windows studio window unavailable");ok=0;
            }else{
                ShowWindow(hwnd,SW_SHOW);UpdateWindow(hwnd);
                MSG message;
                while(GetMessageA(&message,NULL,0,0)>0){
                    TranslateMessage(&message);
                    DispatchMessageA(&message);
                }
                ok=message.message==WM_QUIT;
            }
        }
    }
    free(m->events);free(m);free(ui);
    return ok;
}
