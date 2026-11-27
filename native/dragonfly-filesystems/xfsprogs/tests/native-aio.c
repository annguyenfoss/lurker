#define _POSIX_C_SOURCE 200809L
#include <aio.h>
#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>
int main(int argc,char **argv) {
 assert(argc==2 && getuid()==1300 && geteuid()==1300);
 int fd=open(argv[1],O_CREAT|O_EXCL|O_RDWR,0600);assert(fd>=0);
 char input[]="Genuine native DragonFly lio_listio write/fsync/read control.\n";
 struct aiocb write_req={.aio_fildes=fd,.aio_buf=input,.aio_nbytes=sizeof(input),.aio_lio_opcode=LIO_WRITE};struct aiocb *writes[]={&write_req};
 assert(lio_listio(LIO_WAIT,writes,1,NULL)==0);assert(aio_error(&write_req)==0);assert(aio_return(&write_req)==(ssize_t)sizeof(input));assert(fsync(fd)==0);
 char output[sizeof(input)]={0};struct aiocb read_req={.aio_fildes=fd,.aio_buf=output,.aio_nbytes=sizeof(output),.aio_lio_opcode=LIO_READ};struct aiocb *reads[]={&read_req};
 assert(lio_listio(LIO_WAIT,reads,1,NULL)==0);assert(aio_error(&read_req)==0);assert(aio_return(&read_req)==(ssize_t)sizeof(output));assert(memcmp(input,output,sizeof(input))==0);
 struct stat st;assert(!fstat(fd,&st));assert(st.st_uid==1300 && st.st_gid==1300 && (st.st_mode&0777)==0600);assert(close(fd)==0);
 struct aiocb bad_req={.aio_fildes=-1,.aio_buf=output,.aio_nbytes=sizeof(output),.aio_lio_opcode=LIO_READ};struct aiocb *bad[]={&bad_req};errno=0;int bad_result=lio_listio(LIO_WAIT,bad,1,NULL);int call_errno=errno;int request_error=aio_error(&bad_req);ssize_t bad_bytes=aio_return(&bad_req);assert(bad_result<0 || request_error!=0);assert(bad_bytes<0);
 printf("native AIO actual uid=%lu euid=%lu bytes=%zu mode=0600 write/fsync/read passed; invalid-fd result=%d errno=%d aio_error=%d aio_return=%zd\n",(unsigned long)getuid(),(unsigned long)geteuid(),sizeof(input),bad_result,call_errno,request_error,bad_bytes);return 0;
}
