FROM centos:7

RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
ENV PATH /root/.cargo/bin:$PATH

RUN yum install gcc git -y

# docker build -t kennytm/dbgen-build-env .
