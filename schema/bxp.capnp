@0xabcdef1234567890;

enum Action {
  fetch @0;
  push @1;
  ping @2;
}

struct Request {
  requestId @0 :UInt32;
  action @1 :Action;
  resourceUri @2 :Text;
}

struct Response {
  requestId @0 :UInt32;
  statusCode @1 :StatusCode;
}

enum StatusCode {
  success @0;           # 200 OK equivalent
  badRequest @1;        # 400 equivalent
  unauthorized @2;      # 401 equivalent
  notFound @3;          # 404 equivalent
  internalError @4;     # 500 equivalent
}